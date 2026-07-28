//! The **kind plan table** — the one place that says what a versionable entity *is*
//! (`docs/scope/versions/entity-version-history-scope.md`, "Generic by construction").
//!
//! A kind is a **data row**: `(kind → table, save_tool, how to read its id from a call, whether it
//! carries its own counter)`. Adding dashboards' sibling kinds was adding rows; adding the next one
//! is adding a row. Nothing downstream — capture, list, get, restore, the verbs, the gateway routes
//! — matches on a kind name, so a kind is data everywhere it travels.
//!
//! **This is not a rule-10 leak.** These are *core-owned* verb families (`dashboard.save`,
//! `flows.save`, `rules.save`), not extension ids: the host may name its own verbs. The seam is
//! shaped for extension-declared kinds (a manifest-fed row appended to this table) — deliberately
//! deferred by the scope, but the reason nothing below this file ever sees a `&'static str` kind.

use serde_json::Value;

/// One versionable entity kind.
pub struct KindPlan {
    /// The public kind name callers pass to `versions.*` (`"dashboard"`, `"flow"`, `"rule"`).
    pub kind: &'static str,
    /// The store table its records live in — the table the after-image is read back from, and the
    /// table the snapshot guard vets.
    pub table: &'static str,
    /// The kind's OWN save verb. Capture triggers on it; restore re-dispatches it (never a raw
    /// store write), so every validator, cap, audit hook, and cache invalidation the save already
    /// has applies to a restore for free.
    pub save_tool: &'static str,
    /// The call-argument keys the entity id may arrive under, in order. `rules.save` accepts
    /// `id` **or** `name` (the id defaults to the name), so both are listed — mirroring the verb.
    pub id_keys: &'static [&'static str],
    /// The record field carrying the kind's own monotonic counter, when it has one. Flows keep a
    /// `version: u32` for RUN PINNING (flows-scope Decision 1) — unchanged by this subsystem, but
    /// carried onto each ring row so a UI can finally show what "v12" contained.
    pub version_field: Option<&'static str>,
    /// Does the save verb require a logical `now` the stored record does not carry? `dashboard.save`
    /// does (`required: [... "now"]`); `flows.save` / `rules.save` do not.
    pub needs_now: bool,
    /// Top-level record fields that are **save metadata, not content** — excluded from the dedupe
    /// hash (and from the "current" marker).
    ///
    /// Without this the dedupe never fires for two of the three kinds: a dashboard stamps
    /// `updated_ts` on every save and a flow BUMPS its `version` counter on every save, so
    /// re-saving an unchanged record produced a byte-different snapshot and burned a ring slot —
    /// exactly the no-op-save waste the scope's dedupe exists to prevent. These fields are still
    /// STORED in the snapshot (a restore must write them back); they are only excluded from the
    /// comparison. Declared per kind here, in the plan table, so nothing downstream matches on a
    /// kind name to know it.
    pub hash_ignore: &'static [&'static str],
}

/// Every versionable kind. v1 is the three core-owned entity families the scope names.
pub const KIND_PLANS: &[KindPlan] = &[
    KindPlan {
        kind: "dashboard",
        table: "dashboard",
        save_tool: "dashboard.save",
        id_keys: &["id"],
        version_field: None,
        needs_now: true,
        hash_ignore: &["updated_ts"],
    },
    KindPlan {
        kind: "flow",
        table: "flow",
        save_tool: "flows.save",
        id_keys: &["id"],
        version_field: Some("version"),
        needs_now: false,
        // The run-pinning counter climbs on every save; it is provenance, not graph content.
        hash_ignore: &["version"],
    },
    KindPlan {
        kind: "rule",
        table: "rule",
        save_tool: "rules.save",
        id_keys: &["id", "name"],
        version_field: None,
        needs_now: false,
        // A rule record is all content — nothing on it is stamped by the act of saving.
        hash_ignore: &[],
    },
];

/// The plan for a public kind name, or `None` for an unknown kind (a caller typo → a typed
/// `BadInput`, never a silent store miss — the scope's catalog requirement).
pub fn plan_for_kind(kind: &str) -> Option<&'static KindPlan> {
    KIND_PLANS.iter().find(|p| p.kind == kind)
}

/// The plan whose `save_tool` is `tool`, or `None`. This is how capture classifies a dispatched
/// call without a `match` on verb names.
pub fn plan_for_save_tool(tool: &str) -> Option<&'static KindPlan> {
    KIND_PLANS.iter().find(|p| p.save_tool == tool)
}

/// The table a kind's records live in — the lookup `undo_capture` uses so a `versions.restore` is
/// journaled against the entity it actually rewrites (rather than as an opaque non-generic call).
pub fn table_for_kind(kind: &str) -> Option<&'static str> {
    plan_for_kind(kind).map(|p| p.table)
}

/// What a dispatched call means to version history.
pub struct Captured {
    pub plan: &'static KindPlan,
    /// The entity id the call wrote.
    pub id: String,
}

/// Classify a **successful, depth-0** tool call: does it produce a new version of an entity?
///
/// Two shapes qualify, and only two:
///   - the kind's own `save_tool` — the ordinary path; the id comes from the call's arguments;
///   - `versions.restore` — the restore's own re-dispatch runs at depth+1, *below* the capture
///     chokepoint, so without this arm a restore would silently append no head. The scope calls this
///     out explicitly: the depth-0 wrap must treat the restore as the user action and capture the
///     entity write it performs.
///
/// Everything else (reads, deletes, other verbs) is `None`. There is **no capture on delete** — the
/// ring already holds the last saved states and a tombstone row adds nothing (scope Non-goals).
pub fn classify(qualified_tool: &str, input: &Value) -> Option<Captured> {
    if qualified_tool == crate::versions::RESTORE_TOOL {
        let kind = input.get("kind").and_then(Value::as_str)?;
        let plan = plan_for_kind(kind)?;
        let id = input.get("id").and_then(Value::as_str)?;
        return Some(Captured {
            plan,
            id: id.to_string(),
        });
    }
    let plan = plan_for_save_tool(qualified_tool)?;
    let id = plan
        .id_keys
        .iter()
        .find_map(|k| input.get(*k).and_then(Value::as_str))?;
    Some(Captured {
        plan,
        id: id.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_plan_row_is_coherent() {
        for p in KIND_PLANS {
            assert!(!p.kind.is_empty());
            assert!(!p.table.is_empty());
            assert!(
                p.save_tool.contains('.'),
                "{} needs a dotted save verb so cap wildcards work",
                p.kind
            );
            assert!(!p.id_keys.is_empty(), "{} declares no id key", p.kind);
        }
    }

    #[test]
    fn a_save_is_captured_with_its_kind_and_id() {
        let c = classify(
            "dashboard.save",
            &json!({ "id": "plant-room", "title": "x" }),
        )
        .expect("a dashboard save is captured");
        assert_eq!(c.plan.kind, "dashboard");
        assert_eq!(c.id, "plant-room");

        let c = classify("flows.save", &json!({ "id": "f1", "nodes": [] })).expect("flow captured");
        assert_eq!(c.plan.kind, "flow");
        assert_eq!(c.plan.version_field, Some("version"));
    }

    /// `rules.save` accepts `id` OR `name`; the plan mirrors the verb rather than guessing.
    #[test]
    fn a_rule_save_falls_back_to_name_like_the_verb_does() {
        let c = classify("rules.save", &json!({ "name": "high-temp", "body": "" }))
            .expect("rule captured by name");
        assert_eq!(c.id, "high-temp");
        let c = classify(
            "rules.save",
            &json!({ "id": "r1", "name": "high-temp", "body": "" }),
        )
        .expect("rule captured by id");
        assert_eq!(
            c.id, "r1",
            "an explicit id wins over the name, as in rules.save"
        );
    }

    /// The load-bearing arm: a restore's own save runs at depth+1, below the capture chokepoint, so
    /// the restore CALL is what appends the new head. Without this, restoring v7 would leave the
    /// ring unchanged and "restore appends a new head" would be false.
    #[test]
    fn a_restore_is_captured_as_the_entity_write_it_performs() {
        let c = classify(
            "versions.restore",
            &json!({ "kind": "dashboard", "id": "plant-room", "version_id": "01J" }),
        )
        .expect("a restore is captured");
        assert_eq!(c.plan.kind, "dashboard");
        assert_eq!(c.id, "plant-room");
    }

    #[test]
    fn reads_deletes_and_unknown_kinds_are_not_captured() {
        assert!(classify("dashboard.get", &json!({ "id": "d" })).is_none());
        assert!(classify("dashboard.delete", &json!({ "id": "d" })).is_none());
        assert!(classify("flows.list", &json!({})).is_none());
        assert!(classify("some-ext.write", &json!({ "id": "x" })).is_none());
        assert!(
            classify("versions.restore", &json!({ "kind": "nope", "id": "x" })).is_none(),
            "an unknown kind captures nothing"
        );
        assert!(
            classify("dashboard.save", &json!({ "title": "no id" })).is_none(),
            "a call with no id has nothing to key a ring on"
        );
    }
}
