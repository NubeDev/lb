//! `versions.restore { kind, id, version_id, now? }` — make an old version the live record again.
//!
//! **Restore is a forward action, never a raw store write.** It re-dispatches the kind's OWN save
//! verb with the snapshot as input, one level deeper (`depth + 1`), so it inherits — for free and by
//! construction — every validator the save has (flows' DAG/config/cron checks, dashboard
//! bounds/view checks, the rules schedule compile), the capability wall, ownership, audit, and cache
//! invalidation. The scope litigated and rejected the raw-write alternative; this file is why that
//! decision costs nothing.
//!
//! **No escalation.** The caller must hold the kind's save cap. Restoring is *performing that save*,
//! so a `versions.restore` grant can never reach a mutation the caller could not perform directly —
//! the same rule the undo verb enforces on the original tool's cap.
//!
//! **Restore works after delete.** The ring outlives the entity (there is no capture on delete), so
//! the head snapshot of a deleted dashboard/flow/rule re-creates it: the save verb's create path is
//! the same path.
//!
//! **Last-writer-wins, deliberately.** Unlike undo — which refuses on drift, because an undo asserts
//! a state it observed — a restore asserts an *intent* ("make it look like this again"). Refusing on
//! a concurrent edit would be wrong here, and nothing is lost by racing: the pre-restore state is
//! itself captured as the ring's previous head.

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use lb_store::new_ulid;
use serde::Serialize;
use serde_json::Value;

use crate::tool_call::call_tool_at_depth;
use crate::Node;

use super::error::VersionsError;
use super::list::unknown_kind;
use super::plan::plan_for_kind;
use super::record::ts_of_ulid;
use super::store::read_version;

/// What a restore reports back: enough for a client to refresh the surface and tell the user what
/// happened, without a second read.
#[derive(Debug, Serialize)]
pub struct Restored {
    pub ok: bool,
    pub kind: String,
    pub id: String,
    pub restored_from: String,
    /// The `rev` the snapshot was originally captured at — the "you are now looking at what rev 41
    /// looked like" fact. The NEW rev is whatever the save produced; the caller re-reads the entity.
    pub entity_rev: u64,
}

// Argument count is the explicit dependency list; bundling it into a struct would be a refactor.
#[allow(clippy::too_many_arguments)]
pub async fn versions_restore(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    kind: &str,
    id: &str,
    version_id: &str,
    now: Option<u64>,
    depth: u32,
) -> Result<Restored, VersionsError> {
    // Gate 1: the verb itself.
    authorize_tool(principal, ws, "versions.restore").map_err(|_| VersionsError::Denied)?;
    let plan = plan_for_kind(kind).ok_or_else(|| unknown_kind(kind))?;
    // Gate 2: NO ESCALATION — hold the kind's save cap, checked BEFORE the snapshot is loaded so a
    // caller who may not save also learns nothing about which versions exist.
    authorize_tool(principal, ws, plan.save_tool).map_err(|_| VersionsError::Denied)?;

    let version = read_version(&node.store, ws, kind, id, version_id)
        .await?
        .ok_or(VersionsError::NotFound)?;

    let input = restore_input(&version.snapshot, id, plan.needs_now, now);
    let input_json = input.to_string();

    // The re-dispatch. `depth + 1` keeps it BELOW the depth-0 capture chokepoint on purpose: the
    // user action is `versions.restore`, and it is that call — not this nested save — that undo and
    // version-history capture (see `plan::classify`). Capturing both would double-journal one user
    // action and make Ctrl+Z undo half a restore.
    call_tool_at_depth(node, principal, ws, plan.save_tool, &input_json, depth + 1)
        .await
        .map_err(|e| refusal(plan.save_tool, e))?;

    Ok(Restored {
        ok: true,
        kind: kind.to_string(),
        id: id.to_string(),
        restored_from: version_id.to_string(),
        entity_rev: version.entity_rev,
    })
}

/// Build the save verb's input from a snapshot.
///
/// The snapshot IS the entity's own JSON, and every v1 save verb reads its arguments under the same
/// names the record stores them under — that is not a coincidence, it is the round-trip property
/// that makes the generic seam possible, and the restore-roundtrip test per kind is what holds it.
/// Two things the record cannot carry are added:
///   - `id` — the ring knows which entity this is; a snapshot missing/disagreeing on `id` must not
///     be able to redirect the save at a different record.
///   - `now` — a logical save clock some verbs require and no record carries.
fn restore_input(snapshot: &Value, id: &str, needs_now: bool, now: Option<u64>) -> Value {
    let mut obj = snapshot.as_object().cloned().unwrap_or_default();
    obj.insert("id".into(), Value::String(id.to_string()));
    if needs_now {
        obj.insert("now".into(), Value::from(now.unwrap_or_else(fresh_now_s)));
    }
    Value::Object(obj)
}

/// The logical `now` for a restore whose caller supplied none, in unix SECONDS.
///
/// Derived from a freshly minted ULID's timestamp — the same seam capture uses for `ts` — rather
/// than a direct wall-clock read in a verb. It must move FORWARD (the restored record's
/// `updated_ts` is "when it was restored", not "when the old version was saved"), so the version
/// row's own `ts` is deliberately not reused.
fn fresh_now_s() -> u64 {
    ts_of_ulid(&new_ulid()) / 1000
}

/// Map the re-dispatched save's failure. A denial stays a DENIAL (the caller's caps, opaque); a
/// validator refusal is passed through verbatim so "this old snapshot no longer validates" is
/// legible instead of arriving as a generic failure.
fn refusal(tool: &str, e: ToolError) -> VersionsError {
    match e {
        ToolError::Denied | ToolError::DeniedBecause { .. } => VersionsError::Denied,
        other => VersionsError::RestoreRefused {
            tool: tool.to_string(),
            message: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_ring_s_id_wins_over_the_snapshot_s() {
        // A snapshot that disagrees on `id` must not be able to redirect the save at another record.
        let out = restore_input(
            &json!({ "id": "somewhere-else", "title": "T" }),
            "plant-room",
            false,
            None,
        );
        assert_eq!(out["id"], json!("plant-room"));
        assert_eq!(
            out["title"],
            json!("T"),
            "the rest of the snapshot is untouched"
        );
    }

    #[test]
    fn now_is_added_only_when_the_verb_needs_it() {
        let with = restore_input(&json!({ "title": "T" }), "d", true, Some(1_700_000_000));
        assert_eq!(with["now"], json!(1_700_000_000));
        let without = restore_input(&json!({ "title": "T" }), "d", false, Some(1_700_000_000));
        assert!(
            without.get("now").is_none(),
            "a verb that takes no `now` gets none"
        );
    }

    #[test]
    fn an_absent_now_moves_forward_rather_than_replaying_the_old_save_time() {
        let out = restore_input(&json!({}), "d", true, None);
        let now = out["now"].as_u64().expect("a now was derived");
        assert!(
            now > 1_600_000_000,
            "the derived clock is a real epoch-seconds value"
        );
    }
}
