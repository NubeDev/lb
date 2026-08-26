//! The **reserved-table wall** (ext-store-nodes scope) headless, against a real store (`mem://`).
//! The MCP store-mutate surface (`store.write` / `store.delete`) must reject every host-owned table
//! with a typed `ReservedTable` — checked BEFORE the capability gate, so even the editor bundle's
//! `store:*:write` wildcard (plus the mcp caps) does not pierce it, and there is no override cap.
//! Host internals on the direct `Store` handle are untouched, and `store.tables` flags each row
//! `system: true|false` so the writable-table picker can exclude the wall's tables up front.
//!
//! Includes the **drift test** (scope Risk 1): every known host `TABLE` const is a member of the
//! reserved set, so adding a host table without touching `lb_store::reserved` fails here.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_store_mutate_tool, member_role_caps, store_delete_run, store_tables_view, store_write_run,
    StoreMutateError,
};
use lb_mcp::ToolError;
use lb_store::reserved::{is_reserved, RESERVED_TABLES};
use lb_store::Store;
use serde_json::json;

fn principal(sub: &str, ws: &str, caps: Vec<String>) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps,
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// The strongest legitimate mutate holder: the editor wildcard + both mcp caps. The wall must hold
/// against exactly this principal (scope: "even `store:*:write` must not pierce").
fn wildcard_writer(ws: &str) -> Principal {
    principal(
        "user:editor",
        ws,
        vec![
            "store:*:write".into(),
            "mcp:store.write:call".into(),
            "mcp:store.delete:call".into(),
        ],
    )
}

// ----- the wall: every reserved name, write AND delete, wildcard held ----------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_reserved_table_rejects_write_and_delete_despite_the_wildcard() {
    let ws = "wall";
    let store = Store::memory().await.unwrap();
    let p = wildcard_writer(ws);

    for table in RESERVED_TABLES {
        let err = store_write_run(&store, &p, ws, table, "probe", &json!({ "x": 1 }))
            .await
            .expect_err("reserved write must be rejected");
        assert!(
            matches!(&err, StoreMutateError::ReservedTable { table: t } if t == table),
            "write {table}: expected ReservedTable, got {err:?}"
        );
        let err = store_delete_run(&store, &p, ws, table, "probe")
            .await
            .expect_err("reserved delete must be rejected");
        assert!(
            matches!(&err, StoreMutateError::ReservedTable { table: t } if t == table),
            "delete {table}: expected ReservedTable, got {err:?}"
        );
        // The reject happened before any store touch — nothing was written.
        assert!(
            lb_store::read(&store, ws, table, "probe")
                .await
                .unwrap()
                .is_none(),
            "{table}: no record may land behind the wall"
        );
    }
}

/// Over the MCP bridge the reject is caller-visible author feedback — `BadInput` with the table
/// named — never the opaque `Denied` (the reserved set is a public const; naming it leaks nothing).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn over_mcp_the_reject_is_a_clear_bad_input_not_an_opaque_deny() {
    let ws = "wall-mcp";
    let store = Store::memory().await.unwrap();
    let p = wildcard_writer(ws);

    let args = json!({ "table": "flow", "id": "f1", "value": { "x": 1 } });
    let err = call_store_mutate_tool(&store, &p, ws, "store.write", &args)
        .await
        .expect_err("reserved write over MCP rejected");
    assert!(
        matches!(&err, ToolError::BadInput(m) if m == "reserved table: flow"),
        "expected BadInput(\"reserved table: flow\"), got {err:?}"
    );
    let err = call_store_mutate_tool(&store, &p, ws, "store.delete", &args)
        .await
        .expect_err("reserved delete over MCP rejected");
    assert!(
        matches!(&err, ToolError::BadInput(m) if m == "reserved table: flow"),
        "expected BadInput(\"reserved table: flow\"), got {err:?}"
    );
}

// ----- a non-reserved table with the SAME principal succeeds -------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_user_table_with_the_same_principal_writes_and_deletes() {
    let ws = "wall-user";
    let store = Store::memory().await.unwrap();
    let p = wildcard_writer(ws);

    let value = json!({ "status": "ext down" });
    let (t, id) = store_write_run(&store, &p, ws, "ops_heartbeat", "hb-1", &value)
        .await
        .expect("user-table write succeeds");
    assert_eq!((t.as_str(), id.as_str()), ("ops_heartbeat", "hb-1"));
    assert_eq!(
        lb_store::read(&store, ws, "ops_heartbeat", "hb-1")
            .await
            .unwrap(),
        Some(value),
        "round-trips"
    );
    store_delete_run(&store, &p, ws, "ops_heartbeat", "hb-1")
        .await
        .expect("user-table delete succeeds");
    assert!(
        lb_store::read(&store, ws, "ops_heartbeat", "hb-1")
            .await
            .unwrap()
            .is_none(),
        "erased"
    );
}

// ----- host internals are unaffected (the wall gates the MCP surface only) -----------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn host_internal_writes_through_the_direct_store_handle_still_succeed() {
    let ws = "wall-internal";
    let store = Store::memory().await.unwrap();
    // A flow save is a direct `lb_store::write` into the reserved `flow` table — that path is the
    // host's own and must keep working (packs/migrations/verb families own system-table mutation).
    lb_store::write(&store, ws, "flow", "f1", &json!({ "name": "nightly" }))
        .await
        .expect("host-internal write to a reserved table succeeds");
    let read = lb_store::read(&store, ws, "flow", "f1").await.unwrap();
    assert_eq!(read, Some(json!({ "name": "nightly" })));
    lb_store::delete(&store, ws, "flow", "f1")
        .await
        .expect("host-internal delete succeeds");
}

// ----- drift: every known host TABLE const is a member of the reserved set (scope Risk 1) --------

#[test]
fn every_known_host_table_const_is_reserved() {
    // Programmatic: consts importable from the owning crates / lb_host re-exports.
    let consts: &[&str] = &[
        // flows (the one canonicalized family).
        lb_flows::table::FLOW,
        lb_flows::table::FLOW_RUN,
        lb_flows::table::FLOW_STEP,
        lb_flows::table::FLOW_NODE_STATE,
        lb_flows::table::FLOW_INPUT,
        lb_flows::table::FLOW_TRIGGER_STATE,
        lb_flows::table::FLOW_NODE_MEMORY,
        lb_flows::table::FLOW_NODE_BUFFER,
        // authz / identity plane.
        lb_authz::GRANT_TABLE,
        lb_authz::IDENTITY_TABLE,
        lb_authz::IDENTITY_EMAIL_TABLE,
        lb_authz::IDENTITY_CREDENTIAL_TABLE,
        lb_authz::MEMBERSHIP_TABLE,
        lb_authz::ROLE_TABLE,
        lb_authz::TEAM_TABLE,
        lb_authz::TOKEN_REVOKE_TABLE,
        // durable motion.
        lb_inbox::TABLE,
        lb_inbox::RESOLUTION_TABLE,
        // ingest / series plane.
        lb_ingest::SERIES_TABLE,
        lb_ingest::SERIES_META_TABLE,
        lb_ingest::ROLLUP_TABLE,
        lb_ingest::RETENTION_TABLE,
        lb_ingest::STAGING_TABLE,
        lb_ingest::DEAD_LETTER_TABLE,
        // insights.
        lb_insights::INSIGHT_TABLE,
        lb_insights::NOTIFY_TABLE,
        lb_insights::POLICY_TABLE,
        lb_insights::SUB_TABLE,
        // tags.
        lb_tags::TAG_TABLE,
        lb_tags::TAGGED_TABLE,
        lb_tags::VECTOR_TABLE,
        // prefs / i18n.
        lb_prefs::USER_PREFS_TABLE,
        lb_prefs::WORKSPACE_PREFS_TABLE,
        lb_prefs::CATALOG_TABLE,
        // telemetry.
        lb_telemetry::TABLE,
        // host re-exports.
        lb_host::AGENT_CONFIG_TABLE,
        lb_host::AGENT_DEFS_TABLE,
        lb_host::DECISION_TABLE,
        lb_host::PERSONA_TABLE,
        lb_host::POLICY_TABLE,
        lb_host::APIKEY_TABLE,
        lb_host::EXTRACTION_TABLE,
        lb_host::PACK_RECEIPT_TABLE,
        lb_host::QUERY_TABLE,
        lb_host::WEBHOOK_TABLE,
        lb_host::CHUNK_TABLE,
        // mail sources (mail-source scope).
        lb_host::MAIL_SOURCE_TABLE,
        lb_host::MAIL_IMPORT_TABLE,
    ];
    for t in consts {
        assert!(
            is_reserved(t),
            "host TABLE const {t:?} is missing from lb_store::reserved"
        );
    }

    // Literal list: host-owned tables whose consts are crate-private (each name verified against
    // its owning module — see lb_store::reserved's group comments). If one of these renames, this
    // list is the tripwire.
    for t in [
        "install",          // lb_assets::install
        "asset",            // lb_assets::asset
        "doc",              // lb_assets::doc
        "skill",            // lb_assets::skill (reserved? see below)
        "rel",              // lb_assets::relation
        "registry_catalog", // host registry::catalog
        "registry_cache",   // host registry::cache
        "native_status",    // host native::status
        "dashboard",
        "form", // host forms::model::TABLE
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
        "workspace",
        "user",
        "credential",
        "webhook",
        "invite", // lb_authz::invite (INVITE_TABLE is exported; kept literal beside its claim)
        "invite_claim", // lb_authz::invite
        "secret", // lb_secrets (private const)
        "agent_definition",
        "agent_memory",
        "agent_policy",
        "agent_decision",
        "workspace_agent_config",
        "persona",
        "rule",
        "insight",
        "insight_occ",
        "approval_held_change", // host outbox::enqueue_held
        "proof_sim_change",     // host outbox::enqueue
        "media",
        "media_chunk",
        "datasource",
        "db_schema",
        "extraction",
        "query",
        "device",
        "push_delivered",
        "job",      // lb_jobs (private const)
        "outbox",   // lb_outbox (private const)
        "reminder", // lb_reminders (private const)
        "undo",
        "undo_stack",
        "undo_seq",
        "undo_live",
        "message_catalog",
    ] {
        if t == "skill" {
            // `skill` records live under the shared-asset store surface with its own cap grammar
            // (`store:skill/**`), not the generic table CRUD — deliberately NOT in the reserved set
            // today; this branch documents the exception so a future move is a conscious one.
            continue;
        }
        assert!(
            is_reserved(t),
            "host table literal {t:?} is missing from lb_store::reserved"
        );
    }
}

// ----- store.tables: the system flag + the editor (member) role can call it ----------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn store_tables_flags_system_rows_and_opens_to_the_member_role() {
    let ws = "wall-tables";
    let store = Store::memory().await.unwrap();
    // Seed one reserved row (host-internal write) and one user row so both table kinds exist.
    lb_store::write(&store, ws, "flow", "f1", &json!({ "name": "n" }))
        .await
        .unwrap();
    lb_store::write(&store, ws, "ops_heartbeat", "hb", &json!({ "ok": true }))
        .await
        .unwrap();

    // The EDITOR (member) role bundle — not admin — can call store.tables (ext-store-nodes scope).
    let member = principal("user:author", ws, member_role_caps());
    let tables = store_tables_view(&store, &member, ws)
        .await
        .expect("a member-role principal can call store.tables");

    let find = |name: &str| {
        tables
            .iter()
            .find(|t| t.table == name)
            .unwrap_or_else(|| panic!("{name} missing from store.tables: {tables:?}"))
    };
    let flow = find("flow");
    assert!(flow.system, "flow is host-owned → system: true");
    assert_eq!(flow.count, 1);
    let hb = find("ops_heartbeat");
    assert!(!hb.system, "a user table → system: false");
    assert_eq!(hb.count, 1);

    // The flag is a GLOBAL const property — identical across workspaces (scope isolation test).
    let ws2 = "wall-tables-b";
    lb_store::write(&store, ws2, "flow", "f9", &json!({ "name": "other" }))
        .await
        .unwrap();
    let member2 = principal("user:other", ws2, member_role_caps());
    let tables2 = store_tables_view(&store, &member2, ws2).await.unwrap();
    let flow2 = tables2
        .iter()
        .find(|t| t.table == "flow")
        .expect("ws2 flow row");
    assert_eq!(
        flow2.system, flow.system,
        "system flag identical across workspaces"
    );
    // ...and ws-B's listing never contains ws-A's user table (the hard wall).
    assert!(
        !tables2.iter().any(|t| t.table == "ops_heartbeat"),
        "ws-B must not list ws-A's tables"
    );
}

/// A principal WITHOUT `mcp:store.tables:call` stays opaquely denied — opening the verb to the
/// member role must not open it to everyone.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn store_tables_still_denies_a_caller_without_the_cap() {
    let ws = "wall-tables-deny";
    let store = Store::memory().await.unwrap();
    let nocap = principal("user:mallory", ws, vec!["mcp:store.query:call".into()]);
    let err = store_tables_view(&store, &nocap, ws)
        .await
        .expect_err("no cap → denied");
    assert!(
        matches!(err, lb_host::DbViewError::Denied),
        "opaque deny: {err:?}"
    );
}
