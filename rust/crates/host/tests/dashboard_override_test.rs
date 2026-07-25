//! The dashboard admin-override triad, headless (ext-managed-dashboards scope, Goal 3 / D2) — a real
//! `Store`, real capability sets, real principals, **no fakes** (testing-scope §"no mocks").
//!
//! `dashboard.save_any` and `dashboard.share_any` are asserted exactly the way the shipped
//! `dashboard.delete_any` is (`dashboard_test.rs::delete_any_cap_lets_a_non_owner_admin_delete`):
//! a non-owner with only the base cap is DENIED, the same principal also holding the override
//! SUCCEEDS, and the denied attempt wrote nothing. Plus the check-ORDER assertions the scope calls
//! for — the owner path runs first (an owner holding no override succeeds), and an override is never
//! a bypass of the verb's own base capability (gate 2 still precedes it).
//!
//! The boards under test are extension-MANAGED (`owner = "ext:modbus"`), because that is the case
//! the overrides exist for: an admin must be able to fix and re-scope a board a machine generates,
//! not only delete it. The marker and the owner must survive the override — an admin fix is not a
//! takeover.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    dashboard_get, dashboard_save, dashboard_share, DashboardError, DashboardVisibility,
};
use lb_store::Store;

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

// ── Goal 3 / D2 — the admin-override triad ────────────────────────────────────────────────────

/// `dashboard.save_any`: a non-owner admin holding only the base SAVE cap stays denied; the same
/// principal also holding the override succeeds — and the board's `managedBy`/`owner` are PRESERVED
/// (an admin fix is not a takeover). Mirrors `delete_any_cap_lets_a_non_owner_admin_delete`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_any_cap_lets_a_non_owner_admin_save_a_managed_board() {
    let ws = "ws-managed-save-any";
    let store = Store::memory().await.unwrap();
    let ext = principal("ext:modbus", ws, BASE);
    shared_board(&store, &ext, ws, "board").await;

    // Without the override — denied, and nothing was written.
    let admin = principal("user:admin", ws, BASE);
    assert!(
        dashboard_save(&store, &admin, ws, "board", "Fixed", vec![], vec![], 2)
            .await
            .is_err(),
        "a non-owner without save_any may not overwrite"
    );
    assert_eq!(
        dashboard_get(&store, &ext, ws, "board")
            .await
            .unwrap()
            .title,
        "Devices",
        "the denied save did not write"
    );

    // With the override — allowed, marker and owner intact.
    let admin = principal("user:admin", ws, &[GET, LIST, SAVE, SHARE, SAVE_ANY]);
    let saved = dashboard_save(&store, &admin, ws, "board", "Fixed", vec![], vec![], 3)
        .await
        .expect("save_any lets an admin fix a board they do not own");
    assert_eq!(saved.title, "Fixed");
    assert_eq!(
        saved.owner, "ext:modbus",
        "owner preserved — not a takeover"
    );
    assert_eq!(saved.managed_by, "modbus", "marker preserved — not blanked");
    assert_eq!(
        saved.visibility,
        DashboardVisibility::Workspace,
        "visibility preserved"
    );
}

/// `dashboard.share_any`: the same story for re-scoping a board's visibility — the half that would
/// otherwise be rediscovered the first time an admin needs to narrow an over-shared board (D2).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn share_any_cap_lets_a_non_owner_admin_reshare_a_managed_board() {
    let ws = "ws-managed-share-any";
    let store = Store::memory().await.unwrap();
    let ext = principal("ext:modbus", ws, BASE);
    shared_board(&store, &ext, ws, "board").await;

    let admin = principal("user:admin", ws, BASE);
    assert!(matches!(
        dashboard_share(
            &store,
            &admin,
            ws,
            "board",
            DashboardVisibility::Private,
            None,
            2
        )
        .await
        .unwrap_err(),
        DashboardError::Denied
    ));
    assert_eq!(
        dashboard_get(&store, &ext, ws, "board")
            .await
            .unwrap()
            .visibility,
        DashboardVisibility::Workspace,
        "the denied share did not re-scope it"
    );

    let admin = principal("user:admin", ws, &[GET, LIST, SAVE, SHARE, SHARE_ANY]);
    let shared = dashboard_share(
        &store,
        &admin,
        ws,
        "board",
        DashboardVisibility::Private,
        None,
        3,
    )
    .await
    .expect("share_any lets an admin re-scope a board they do not own");
    assert_eq!(shared.visibility, DashboardVisibility::Private);
    assert_eq!(shared.owner, "ext:modbus", "owner preserved");
    assert_eq!(shared.managed_by, "modbus", "marker preserved");
}

/// **Check order.** The owner path runs FIRST and the override is attempted only when it fails, and
/// neither override is a bypass of the verb's own base capability (gate 2 precedes both):
///   - an OWNER holding neither `save_any` nor `share_any` succeeds → the override was never needed;
///   - a non-owner holding ONLY the override (no base `save`/`share` cap) is denied at gate 2.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn owner_path_runs_first_and_the_override_never_bypasses_gate_two() {
    let ws = "ws-managed-order";
    let store = Store::memory().await.unwrap();
    let ext = principal("ext:modbus", ws, BASE);
    shared_board(&store, &ext, ws, "board").await;

    // Owner, no override caps at all — succeeds on the owner path alone.
    dashboard_save(&store, &ext, ws, "board", "Devices v2", vec![], vec![], 2)
        .await
        .expect("the owner never consults the override");
    dashboard_share(
        &store,
        &ext,
        ws,
        "board",
        DashboardVisibility::Workspace,
        None,
        2,
    )
    .await
    .expect("the owner never consults the override");

    // Non-owner holding ONLY the override — gate 2 (`dashboard.save`/`.share`) fires first, so the
    // override alone grants nothing. An `*_any` cap widens WHOSE assets the verb reaches, never
    // WHETHER the caller may call the verb.
    let only_override = principal("user:admin", ws, &[GET, LIST, SAVE_ANY, SHARE_ANY]);
    assert!(matches!(
        dashboard_save(&store, &only_override, ws, "board", "X", vec![], vec![], 3)
            .await
            .unwrap_err(),
        DashboardError::Denied
    ));
    assert!(matches!(
        dashboard_share(
            &store,
            &only_override,
            ws,
            "board",
            DashboardVisibility::Private,
            None,
            3
        )
        .await
        .unwrap_err(),
        DashboardError::Denied
    ));
}
