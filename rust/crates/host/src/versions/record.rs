//! The `entity_version` ring row + the stable snapshot hash the dedupe compares
//! (`docs/scope/versions/entity-version-history-scope.md`, "Storage").
//!
//! One row = one full after-image of one entity at one save. Rows live in a `capped_insert` ring
//! keyed `"{kind}:{id}"`, so the store trims to the newest N in the same transaction that writes the
//! newest one (`crates/store/src/capped.rs` — the module doc is the retention design).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The store table the ring lives in. Reserved (`lb_store::RESERVED_TABLES`): a `store.write` holder
/// must not be able to forge a version row — a forged snapshot becomes a write to the real entity
/// the moment someone restores it.
pub const TABLE: &str = "entity_version";

/// The FIFO bucket one entity's ring occupies. Per-entity, so a chatty dashboard cannot evict a
/// quiet flow's history (the `cap_key` selector `capped_insert` was built for).
pub fn cap_key(kind: &str, id: &str) -> String {
    format!("{kind}:{id}")
}

/// One stored version. `snapshot` is the entity's own JSON — the `data` half of the store envelope,
/// exactly what its save verb would accept back.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityVersion {
    /// The ULID that is both the record id and the ring's FIFO `seq`.
    pub version_id: String,
    pub kind: String,
    pub entity_id: String,
    /// The store `rev` this snapshot was read at — the provenance a reviewer checks.
    pub entity_rev: u64,
    /// The kind's own counter, when it has one (flows' run-pinning `version`). `None` otherwise.
    #[serde(default)]
    pub entity_version: Option<u64>,
    /// The verb that produced it (`dashboard.save`, or `versions.restore` for a restore's head).
    pub tool: String,
    /// The principal that saved it.
    pub actor: String,
    /// Unix MILLIS, decoded from the ULID rather than read from a clock — see [`ts_of_ulid`].
    pub ts: u64,
    /// The stable content hash the dedupe compares (see [`snapshot_hash`]).
    pub hash: String,
    pub snapshot: Value,
}

/// The metadata projection `versions.list` returns. **Never carries `snapshot`** — the scope's
/// "list never ships N full snapshots in one response"; a client fetches one lazily with
/// `versions.get`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMeta {
    pub version_id: String,
    pub kind: String,
    pub entity_id: String,
    pub entity_rev: u64,
    pub entity_version: Option<u64>,
    pub tool: String,
    pub actor: String,
    pub ts: u64,
    pub hash: String,
    /// Does this version's content equal the entity's CURRENT content? Marks the "current" row in a
    /// history list. Computed per read against the live record — never stored (it would go stale the
    /// moment the entity changed).
    pub is_head: bool,
}

impl EntityVersion {
    pub fn meta(&self, is_head: bool) -> VersionMeta {
        VersionMeta {
            version_id: self.version_id.clone(),
            kind: self.kind.clone(),
            entity_id: self.entity_id.clone(),
            entity_rev: self.entity_rev,
            entity_version: self.entity_version,
            tool: self.tool.clone(),
            actor: self.actor.clone(),
            ts: self.ts,
            hash: self.hash.clone(),
            is_head,
        }
    }
}

/// The millisecond timestamp encoded in a ULID's first 48 bits.
///
/// **Why the id and not a clock.** Core verbs take their logical `now` from call arguments and never
/// read a wall clock (the determinism discipline the undo journal follows by writing `ts: 0`). But a
/// version list is a *human* surface — "2 minutes ago — ada" is the whole point — and two of the
/// three v1 save verbs carry no `now` argument to borrow. A ULID already encodes the mint time, and
/// the ring already mints one per row for FIFO ordering, so decoding it costs nothing, adds no clock
/// call to any verb, and cannot disagree with the ring's own ordering. Recorded as a judgment call
/// in the session doc.
pub fn ts_of_ulid(id: &str) -> u64 {
    lb_store::ulid_timestamp_ms(id)
}

/// A stable content hash of a snapshot, used **only** for dedupe and the "current" marker.
///
/// `ignore` names top-level fields that are save METADATA rather than content (a dashboard's
/// `updated_ts`, a flow's run-pinning `version`) — see `KindPlan::hash_ignore` for why excluding
/// them is what makes dedupe fire at all. They stay in the stored snapshot; only the comparison
/// skips them.
///
/// FNV-1a over a canonical rendering (object keys sorted at every depth), not `DefaultHasher`:
/// these hashes are persisted in ring rows and compared across node restarts and Rust releases, and
/// `DefaultHasher`'s output is explicitly not stable across either. Canonical key order matters
/// because the store's JSON map preserves insertion order, so the same record can round-trip with
/// its keys in a different order and must still hash equal.
pub fn snapshot_hash(v: &Value, ignore: &[&str]) -> String {
    let mut buf = String::new();
    match (v.as_object(), ignore.is_empty()) {
        (Some(map), false) => {
            let stripped: serde_json::Map<String, Value> = map
                .iter()
                .filter(|(k, _)| !ignore.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            canonical(&Value::Object(stripped), &mut buf);
        }
        _ => canonical(v, &mut buf),
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in buf.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Render `v` deterministically: object keys sorted, arrays in order, scalars via their JSON form.
fn canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for k in keys {
                out.push_str(k);
                out.push(':');
                canonical(&map[k], out);
                out.push(',');
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for i in items {
                canonical(i, out);
                out.push(',');
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_order_does_not_change_the_hash() {
        let a = json!({ "a": 1, "b": { "x": [1, 2], "y": "z" } });
        let b = json!({ "b": { "y": "z", "x": [1, 2] }, "a": 1 });
        assert_eq!(snapshot_hash(&a, &[]), snapshot_hash(&b, &[]));
    }

    #[test]
    fn a_real_change_changes_the_hash() {
        let a = json!({ "title": "Plant Room", "cells": [] });
        let b = json!({ "title": "Plant Rooms", "cells": [] });
        assert_ne!(snapshot_hash(&a, &[]), snapshot_hash(&b, &[]));
    }

    /// Array ORDER is content (a reordered dashboard is a different dashboard), unlike key order.
    #[test]
    fn array_order_is_content() {
        assert_ne!(
            snapshot_hash(&json!([1, 2]), &[]),
            snapshot_hash(&json!([2, 1]), &[])
        );
    }

    #[test]
    fn ts_comes_from_the_ulid_not_a_clock() {
        let id = lb_store::new_ulid();
        assert!(
            ts_of_ulid(&id) > 1_600_000_000_000,
            "a fresh ULID decodes to a real epoch-millis"
        );
        assert_eq!(
            ts_of_ulid("not-a-ulid"),
            0,
            "an undecodable id degrades to 0, never a panic"
        );
    }

    /// The dedupe's load-bearing property: a record that differs ONLY in a save-stamped field
    /// hashes equal, so a no-op re-save does not burn a ring slot.
    #[test]
    fn ignored_fields_do_not_change_the_hash() {
        let a = json!({ "title": "Ops", "cells": [], "updated_ts": 1 });
        let b = json!({ "title": "Ops", "cells": [], "updated_ts": 999 });
        assert_ne!(snapshot_hash(&a, &[]), snapshot_hash(&b, &[]));
        assert_eq!(
            snapshot_hash(&a, &["updated_ts"]),
            snapshot_hash(&b, &["updated_ts"])
        );
        // ...but a REAL change still differs, ignore list or not.
        let c = json!({ "title": "Other", "cells": [], "updated_ts": 1 });
        assert_ne!(
            snapshot_hash(&a, &["updated_ts"]),
            snapshot_hash(&c, &["updated_ts"])
        );
    }

    #[test]
    fn cap_key_is_per_entity() {
        assert_eq!(cap_key("dashboard", "plant-room"), "dashboard:plant-room");
        assert_ne!(cap_key("flow", "a"), cap_key("rule", "a"));
    }
}
