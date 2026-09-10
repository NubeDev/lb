//! The canonical **reserved-table set** — every store table the host itself owns (ext-store-nodes
//! scope, "the reserved-table wall"). One const slice, one predicate, one owner: before this module
//! the host-owned names were scattered across per-crate `TABLE` consts, and nothing stopped a
//! `store.write` holder (`store:*:write` is in the editor bundle) from overwriting `flow` /
//! `install` / `dashboard` rows and bricking the node. The wall in the host's store-mutate surface
//! rejects any table named here — **regardless of capability grants**, with no override cap
//! (a legitimate need to mutate a system table is an admin/host feature: packs, migrations, the
//! owning verb family — never a generic `store.write`).
//!
//! What is **NOT** reserved: extension-owned and user/pack tables (`site`, `point_reading`,
//! `ems_*`, …) — that is exactly the data the generic store CRUD surface exists to serve. The set
//! is **global** (a const, not state) and identical across workspaces.
//!
//! Drift guard: owning modules keep their own `TABLE` consts; a host-side unit test walks every
//! known const (and `lb_packs::RESERVED_CORE_TABLES`) and asserts membership here, so adding a
//! host table without touching this file fails CI (scope Risk 1). Host internals are unaffected —
//! they write through the direct `Store` handle (`lb_store::write`), never through `store.write`.

/// Every host-owned table, grouped as in the ext-store-nodes scope. Names the scope enumerates are
/// kept verbatim even where no code table exists yet (`member`, `share` — the live tables are
/// `membership` and the `rel` share edge); tables the scope missed but the code owns are added
/// (the authz/identity plane, ingest/series, insights internals, motion (inbox/outbox/jobs),
/// undo/history, prefs/i18n, tags, telemetry — see the group comments).
pub const RESERVED_TABLES: &[&str] = &[
    // -- flow family (lb_flows::table) --
    "flow",
    "flow_run",
    "flow_step_output",
    "flow_node_state",
    "flow_input",
    "flow_trigger_state",
    "flow_node_memory",
    "flow_node_buffer",
    // -- install / registry --
    "install",
    "registry_catalog",
    "registry_cache",
    "native_status",
    "pack_receipt",
    // -- dashboard / UI --
    "dashboard",
    "form",
    "panel",
    "nav",
    "nav_pref",
    "nav_hidden",
    "nav_ext_boards",
    "workspace_nav_default",
    "ui_layout",
    "channel_registry",
    "channel_chart_pref",
    "render_template",
    "report",
    "brand",
    // -- mail sources (mail-source scope) --
    // A forged `mail_source` row would aim a poller at an arbitrary host WITH the workspace's own
    // sealed credentials; a forged `mail_import` row would make a real message permanently
    // un-importable. Both are host-owned, and neither is reachable through generic `store.write`.
    "mail_source",
    "mail_import",
    // -- auth / identity (scope list + the authz-plane tables the scope missed) --
    "workspace",
    "user",
    "apikey",
    "credential",
    "member",
    "share",
    "webhook",
    "membership",
    "identity",
    "identity_email",
    "identity_credential",
    "invite",
    "invite_claim",
    "grant",
    "role",
    "team",
    "token_revoke",
    "secret",
    "gateway",
    // -- agent / rules / insights (scope list + the insight internals the scope missed) --
    "agent_definition",
    "agent_memory",
    "agent_policy",
    "agent_decision",
    "workspace_agent_config",
    "persona",
    "rule",
    "insight",
    "insight_occ",
    "insight_notify",
    "insight_policy",
    "insight_sub",
    "approval_held_change",
    "proof_sim_change",
    // -- data / media (scope list + tags, the scope missed them) --
    "doc",
    "asset",
    "rel",
    "media",
    "media_chunk",
    "datasource",
    "db_schema",
    "extraction",
    "query",
    "device",
    "push_delivered",
    "tag",
    "tagged",
    "tag_vector",
    // -- ingest / series plane (host-owned; writes go through `ingest.write`, never `store.write`) --
    "series",
    "series_meta",
    "series_rollup",
    "series_retention",
    // The last retention GC pass, one row per workspace — host bookkeeping written by `run_gc`
    // alone, read by `series.retention.status` (series-observability scope).
    "series_gc_pass",
    // Retired: ingest no longer stages, but an upgraded node can still carry rows under this name,
    // so the name stays walled off rather than becoming claimable user data.
    "ingest_staging",
    "ingest_dead_letter",
    // -- durable motion (inbox/outbox/jobs) + reminders --
    "inbox",
    "resolution",
    "outbox",
    "job",
    "reminder",
    // -- undo / history --
    "undo",
    "undo_stack",
    "undo_seq",
    "undo_live",
    // -- entity version history (versions scope) --
    // The per-entity snapshot ring + its per-workspace cap override. Host-owned like the undo
    // journal beside it: a `store.write` holder must not be able to forge or blank a version row
    // (a forged snapshot is a write to the real entity the moment someone restores it).
    "entity_version",
    "versions_config",
    // -- node update audit (node-update scope) --
    // "who replaced the binary on this box" must survive the binary — and must not be forgeable or
    // erasable through the generic store CRUD surface. An audit trail a `store:*:write` holder can
    // rewrite is worth nothing.
    "update_audit",
    // -- prefs / i18n --
    "user_prefs",
    "workspace_prefs",
    "message_catalog",
    // -- telemetry --
    "telemetry",
];

/// Is `table` host-owned (reserved)? Exact-name match — the store namespaces by workspace, not by
/// table-name pattern, so there is nothing to glob. The MCP store-mutate surface rejects a reserved
/// table before its capability gate; host internals on the direct `Store` handle are unaffected.
pub fn is_reserved(table: &str) -> bool {
    RESERVED_TABLES.contains(&table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_names_hit_and_user_tables_miss() {
        for t in [
            "flow",
            "install",
            "dashboard",
            "workspace",
            "series",
            "undo",
        ] {
            assert!(is_reserved(t), "{t} must be reserved");
        }
        for t in [
            "site",
            "point_reading",
            "ems_meter",
            "ops_heartbeat",
            "widget",
        ] {
            assert!(!is_reserved(t), "{t} must NOT be reserved (user/ext data)");
        }
    }

    /// Exact-name semantics: no prefix/suffix/case creep — `flows` or `Flow` is an ordinary table.
    #[test]
    fn matching_is_exact() {
        assert!(is_reserved("flow"));
        for t in ["flows", "flow_", "Flow", "FLOW", " flow", "my_flow"] {
            assert!(!is_reserved(t), "{t} must not match by fuzz");
        }
    }

    /// The list is duplicate-free — a duplicate is a merge-artifact smell.
    #[test]
    fn list_has_no_duplicates() {
        let mut seen = std::collections::BTreeSet::new();
        for t in RESERVED_TABLES {
            assert!(seen.insert(*t), "duplicate reserved table: {t}");
        }
    }
}
