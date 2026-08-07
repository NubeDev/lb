//! The managed-dashboard MARKER, headless (ext-managed-dashboards scope, Goals 2/5 + D1/D3) — a
//! real `Store`, real capability sets, real principals, **no fakes** (testing-scope §"no mocks").
//! The admin-override triad (Goal 3 / D2) is the sibling file, `dashboard_override_test.rs`.
//!
//! The mandatory categories this file owns, each in its own test:
//!   - **marker provenance** — a human cannot set `managedBy`, an extension principal always gets
//!     it, and `ext:a` cannot save over `ext:b`'s board;
//!   - **workspace isolation** — a managed board in ws A is invisible and unsavable from ws B;
//!   - **round-trip** — a PRE-FIELD stored record (written raw, no `managedBy` key) reads with an
//!     empty marker and re-saves without losing a field; `DashboardSummary` carries the marker;
//!   - **denial shape** — the typed managed-denial reaches only a caller who could already READ the
//!     board; everyone else still gets the opaque `Denied` (no existence leak).
//!
//! The extension principal is the real one a native sidecar's callback token carries:
//! `sub = "ext:<ext_id>"` (`host/src/native/spec.rs::mint_child_token`).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_dashboard_tool, dashboard_get, dashboard_list, dashboard_save, dashboard_share,
    DashboardError, DashboardVisibility,
};
use lb_store::Store;
use serde_json::{json, Value};

/// A principal `sub` in workspace `ws` holding `caps` — minted and VERIFIED, never hand-built.
fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const GET: &str = "mcp:dashboard.get:call";
const LIST: &str = "mcp:dashboard.list:call";
const SAVE: &str = "mcp:dashboard.save:call";
const SHARE: &str = "mcp:dashboard.share:call";
const SAVE_ANY: &str = "mcp:dashboard.save_any:call";
const SHARE_ANY: &str = "mcp:dashboard.share_any:call";
const BASE: &[&str] = &[GET, LIST, SAVE, SHARE];

/// The store table dashboards live in (`dashboard/model.rs::TABLE`) — used only by the pre-field
/// round-trip test, which writes a RAW record the way an older build left it on disk.
const TABLE: &str = "dashboard";

/// Save a workspace-visible board owned by `p` — the shape an extension publishes (create private,
/// then share to the workspace; both owner-only, and it IS the owner).
async fn shared_board(store: &Store, p: &Principal, ws: &str, id: &str) {
    dashboard_save(store, p, ws, id, "Devices", vec![], vec![], 1)
        .await
        .expect("owner creates");
    dashboard_share(store, p, ws, id, DashboardVisibility::Workspace, None, 1)
        .await
        .expect("owner shares");
}

// ── Goal 2 / D1 — the marker is derived from the principal ────────────────────────────────────

/// An extension principal (`sub = "ext:modbus"`) ALWAYS gets the marker, and it is the BARE id (D1) —
/// the full principal is already on `owner`. A human's board is unmarked.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn extension_principal_stamps_the_bare_id_a_human_stamps_nothing() {
    let ws = "ws-managed-stamp";
    let store = Store::memory().await.unwrap();

    let ext = principal("ext:modbus", ws, BASE);
    let d = dashboard_save(
        &store,
        &ext,
        ws,
        "modbus-sdm630",
        "SDM630",
        vec![],
        vec![],
        1,
    )
    .await
    .unwrap();
    assert_eq!(d.managed_by, "modbus", "bare id, not the ext: principal");
    assert_eq!(d.owner, "ext:modbus", "the full principal stays on owner");

    let test = principal("user:test", ws, BASE);
    let d = dashboard_save(&store, &test, ws, "ops", "Ops", vec![], vec![], 1)
        .await
        .unwrap();
    assert_eq!(d.managed_by, "", "a human board is not managed");
}

/// The marker is DERIVED, never accepted: a human sending `managedBy` through the real MCP bridge
/// has it ignored — the stored record is unmarked, so nobody can forge a badge.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_human_cannot_set_managed_by_through_the_mcp_surface() {
    let ws = "ws-managed-forge";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, BASE);

    let out = call_dashboard_tool(
        &store,
        &test,
        ws,
        "dashboard.save",
        &json!({ "id": "ops", "title": "Ops", "cells": [], "now": 1, "managedBy": "modbus" }),
    )
    .await
    .expect("the save itself succeeds — the bogus field is simply not read");
    assert_eq!(out["managedBy"], json!(""), "input marker ignored");

    let stored = dashboard_get(&store, &test, ws, "ops").await.unwrap();
    assert_eq!(stored.managed_by, "", "and it never reached the record");
}

/// `ext:a` cannot save over `ext:b`'s board (the owner check refuses it), and the marker survives
/// the attempt — one extension can neither overwrite nor CLAIM another's board.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn one_extension_cannot_save_over_anothers_board() {
    let ws = "ws-managed-crossext";
    let store = Store::memory().await.unwrap();
    let ext_b = principal("ext:bacnet", ws, BASE);
    shared_board(&store, &ext_b, ws, "board").await;

    let ext_a = principal("ext:modbus", ws, BASE);
    let err = dashboard_save(&store, &ext_a, ws, "board", "Stolen", vec![], vec![], 2)
        .await
        .unwrap_err();
    // ext:a CAN read the workspace-visible board, so it gets the typed refusal naming ext:b — and
    // that is the whole information: it may not write it.
    assert!(
        matches!(&err, DashboardError::ManagedDenied(id) if id == "bacnet"),
        "expected the typed managed denial, got {err:?}"
    );

    let after = dashboard_get(&store, &ext_b, ws, "board").await.unwrap();
    assert_eq!(after.managed_by, "bacnet", "marker unchanged");
    assert_eq!(after.title, "Devices", "content unchanged");
}

// ── Workspace isolation (mandatory) ───────────────────────────────────────────────────────────

/// A managed board in ws A is invisible AND unsavable from ws B — the workspace wall is structural
/// and the marker changes nothing about it. Also: an `ext:modbus` principal scoped to ws B has no
/// reach into ws A's board of the same id, so a shared extension identity is not a cross-ws bridge.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_managed_board_is_invisible_and_unsavable_across_workspaces() {
    let store = Store::memory().await.unwrap();
    let ext_a = principal("ext:modbus", "ws-a", BASE);
    shared_board(&store, &ext_a, "ws-a", "board").await;

    for sub in ["user:ben", "ext:modbus"] {
        let outsider = principal(sub, "ws-b", &[GET, LIST, SAVE, SHARE, SAVE_ANY, SHARE_ANY]);
        assert!(
            matches!(
                dashboard_get(&store, &outsider, "ws-b", "board")
                    .await
                    .unwrap_err(),
                DashboardError::NotFound
            ),
            "{sub} in ws-b must not see ws-a's managed board"
        );
        assert!(
            dashboard_list(&store, &outsider, "ws-b")
                .await
                .unwrap()
                .is_empty(),
            "{sub}'s ws-b roster must be empty"
        );
        // A save in ws-b CREATES a ws-b record; it cannot touch ws-a's. (A distinct id per subject
        // so the second pass still starts from an empty ws-b roster of readable boards.)
        dashboard_save(
            &store,
            &outsider,
            "ws-b",
            &format!("board-{sub}"),
            "Other",
            vec![],
            vec![],
            4,
        )
        .await
        .unwrap();
        assert_eq!(
            dashboard_get(&store, &ext_a, "ws-a", "board")
                .await
                .unwrap()
                .title,
            "Devices",
            "ws-a's board is untouched by {sub}'s ws-b save"
        );
    }
}

// ── Round-trip (mandatory) ────────────────────────────────────────────────────────────────────

/// A PRE-FIELD stored record — written raw with no `managedBy` key, exactly as an older build left
/// it — reads with an empty marker, keeps every other field, and re-saves without loss. Additive +
/// defaulted, no migration.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_pre_field_record_reads_empty_and_re_saves_clean() {
    let ws = "ws-managed-roundtrip";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, BASE);

    // Build a real record, then strip the key to produce the pre-field bytes and write them RAW.
    let made = dashboard_save(&store, &test, ws, "ops", "Ops", vec![], vec![], 1)
        .await
        .unwrap();
    let mut raw = serde_json::to_value(&made).unwrap();
    raw.as_object_mut().unwrap().remove("managedBy");
    assert!(raw.get("managedBy").is_none(), "pre-field bytes");
    lb_store::write(&store, ws, TABLE, "ops", &raw)
        .await
        .unwrap();

    // Reads clean, with every other field intact.
    let read = dashboard_get(&store, &test, ws, "ops").await.unwrap();
    assert_eq!(read.managed_by, "", "absent key defaults to unmanaged");
    assert_eq!(read, made, "every other field round-trips unchanged");

    // An explicit `null` (the shape an AI/JS writer emits) is tolerated identically.
    let mut nulled = raw.clone();
    nulled
        .as_object_mut()
        .unwrap()
        .insert("managedBy".into(), Value::Null);
    lb_store::write(&store, ws, TABLE, "ops", &nulled)
        .await
        .unwrap();
    assert_eq!(
        dashboard_get(&store, &test, ws, "ops").await.unwrap(),
        made,
        "an explicit null deserializes as the same default"
    );

    // Re-saving the pre-field record loses nothing and stays unmanaged (a human re-save cannot
    // acquire a marker).
    let resaved = dashboard_save(&store, &test, ws, "ops", "Ops", vec![], vec![], 2)
        .await
        .unwrap();
    assert_eq!(resaved.managed_by, "");
    assert_eq!(resaved.owner, made.owner);
}

/// `DashboardSummary` carries the marker (D3) — a roster paints the badge and filters on it without
/// a full `get` per row. Both the managed and the unmanaged row are present and correctly marked.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_roster_summary_carries_the_marker() {
    let ws = "ws-managed-roster";
    let store = Store::memory().await.unwrap();
    let ext = principal("ext:modbus", ws, BASE);
    let test = principal("user:test", ws, BASE);
    shared_board(&store, &ext, ws, "managed").await;
    dashboard_save(&store, &test, ws, "mine", "Mine", vec![], vec![], 1)
        .await
        .unwrap();

    let roster = dashboard_list(&store, &test, ws).await.unwrap();
    let managed = roster.iter().find(|s| s.id == "managed").expect("listed");
    let mine = roster.iter().find(|s| s.id == "mine").expect("listed");
    assert_eq!(managed.managed_by, "modbus");
    assert_eq!(mine.managed_by, "");

    // And on the wire it is `managedBy` (the same key the full record uses).
    let wire = serde_json::to_value(managed).unwrap();
    assert_eq!(wire["managedBy"], json!("modbus"));
}

// ── Goal 5 — the denial shape, without an existence leak ──────────────────────────────────────

/// The typed managed-denial goes ONLY to a caller who could already read the board.
///   - workspace-visible managed board → a non-owner member gets `ManagedDenied("modbus")`;
///   - PRIVATE managed board → the same member gets the OPAQUE `Denied` (it must not learn the
///     board exists, let alone that an extension generates it);
///   - an UNMANAGED board → the opaque `Denied` as before (no behaviour change for human boards).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_managed_denial_is_typed_only_for_a_caller_who_can_read_the_board() {
    let ws = "ws-managed-denial";
    let store = Store::memory().await.unwrap();
    let ext = principal("ext:modbus", ws, BASE);
    let test = principal("user:test", ws, BASE);
    let ben = principal("user:ben", ws, BASE);

    // (1) Readable (workspace-visible) + managed → typed, naming the extension.
    shared_board(&store, &ext, ws, "open").await;
    let err = dashboard_save(&store, &ben, ws, "open", "Edited", vec![], vec![], 2)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, DashboardError::ManagedDenied(id) if id == "modbus"),
        "a reader who tried to save must learn WHY, got {err:?}"
    );
    assert_eq!(
        err.to_string(),
        "denied: managed=modbus",
        "the wire shape the client branches on"
    );

    // (2) UNREADABLE (private) + managed → opaque. This is the no-existence-leak rule: the refusal
    // must be indistinguishable from the one for a board that does not exist at all.
    dashboard_save(&store, &ext, ws, "secret", "Secret", vec![], vec![], 1)
        .await
        .unwrap();
    let err = dashboard_save(&store, &ben, ws, "secret", "Edited", vec![], vec![], 2)
        .await
        .unwrap_err();
    assert!(
        matches!(err, DashboardError::Denied),
        "a caller who cannot READ the board must get the opaque denial, got {err:?}"
    );

    // (3) Unmanaged + readable → opaque, exactly as before this scope.
    dashboard_save(&store, &test, ws, "human", "Human", vec![], vec![], 1)
        .await
        .unwrap();
    dashboard_share(
        &store,
        &test,
        ws,
        "human",
        DashboardVisibility::Workspace,
        None,
        1,
    )
    .await
    .unwrap();
    assert!(matches!(
        dashboard_save(&store, &ben, ws, "human", "Edited", vec![], vec![], 2)
            .await
            .unwrap_err(),
        DashboardError::Denied
    ));
}

/// The same denial over the real MCP bridge: the typed refusal becomes
/// `ToolError::DeniedBecause { code: "managed", subject: "<ext id>" }`, and the unreadable case
/// stays the opaque `ToolError::Denied` — the transport must not widen what the host decided.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_managed_denial_survives_the_mcp_bridge_without_widening() {
    let ws = "ws-managed-denial-mcp";
    let store = Store::memory().await.unwrap();
    let ext = principal("ext:modbus", ws, BASE);
    let ben = principal("user:ben", ws, BASE);
    shared_board(&store, &ext, ws, "open").await;
    dashboard_save(&store, &ext, ws, "secret", "Secret", vec![], vec![], 1)
        .await
        .unwrap();

    let err = call_dashboard_tool(
        &store,
        &ben,
        ws,
        "dashboard.save",
        &json!({ "id": "open", "title": "Edited", "cells": [], "now": 2 }),
    )
    .await
    .unwrap_err();
    assert_eq!(
        err,
        lb_mcp::ToolError::DeniedBecause {
            code: "managed".into(),
            subject: "modbus".into()
        }
    );

    let err = call_dashboard_tool(
        &store,
        &ben,
        ws,
        "dashboard.save",
        &json!({ "id": "secret", "title": "Edited", "cells": [], "now": 2 }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, lb_mcp::ToolError::Denied, "no existence leak over MCP");
}
