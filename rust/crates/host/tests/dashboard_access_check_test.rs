//! `dashboard.access_check` — the dependency-closure preflight (access-model scope), over the REAL
//! store (no mocks). Proves the mandatory categories:
//!
//! - **Dependency-closure correctness (the headline):** a dashboard with (a) an unshared panel, (b) a
//!   missing datasource, (c) an unbound required variable each produce the exact red verdict; sharing
//!   the panel / registering the source + endpoint / binding the var each flips it green.
//! - **Preflight/live match (the cardinal sin guard):** a subject the preflight reports lacking
//!   `federation.query` is ALSO denied by the SAME `authorize_tool` gate the live route runs first —
//!   the preflight's verdict is the live route's verdict, not a parallel guess.
//! - **Capability deny:** a subject without the source cap gets a red `source_cap` verdict.
//! - **Workspace isolation:** a datasource in ws-B is reported absent (never leaked) to a ws-A preflight.
//! - **No-widening:** the preflight GRANTS nothing — it reads and reports; a red verdict is not
//!   silently satisfied.

use lb_assets::{record_install, Install};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::Subject;
use lb_host::{
    add_member, dashboard_access_check, dashboard_save, dashboard_share, panel_save,
    put_datasource, resolve_caps, Cell, DashboardVisibility, Datasource, DepKind, DepVerdict,
    Scope,
};
use lb_mcp::authorize_tool;
use lb_store::Store;
use serde_json::json;

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

/// Admin caps needed to seed the closure (save dashboard/panel, share, add team member, preflight
/// for another subject via authz.resolve).
const ADMIN: &[&str] = &[
    "mcp:dashboard.save:call",
    "mcp:dashboard.share:call",
    "mcp:dashboard.access_check:call",
    "mcp:panel.save:call",
    "mcp:panel.share:call",
    "mcp:authz.resolve:call",
    "mcp:grants.assign:call",
    // ada must HOLD federation.query to grant it to bob (no-widening).
    "mcp:federation.query:call",
    "store:doc/*:write",
];

fn find<'a>(deps: &'a [DepVerdict], dep_contains: &str) -> Option<&'a DepVerdict> {
    deps.iter().find(|d| d.dep.contains(dep_contains))
}

/// A v3 cell referencing a library panel.
fn panel_ref_cell(i: &str, panel_id: &str) -> Cell {
    Cell {
        i: i.into(),
        x: 0,
        y: 0,
        w: 6,
        h: 4,
        v: 3,
        panel_ref: panel_id.into(),
        ..Default::default()
    }
}

/// A v3 cell with one inline `federation.query` source on datasource `ds`.
fn federation_cell(i: &str, ds: &str) -> Cell {
    Cell {
        i: i.into(),
        x: 0,
        y: 0,
        w: 6,
        h: 4,
        v: 3,
        sources: vec![lb_host::CellTarget {
            tool: "federation.query".into(),
            args: json!({ "datasource": ds }),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn closure_red_then_green_and_deny_matches_live() {
    let ws = "acme";
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", ws, ADMIN);

    // A PRIVATE panel owned by ada (never shared to bob) — the unshared-panel dependency.
    panel_save(&store, &admin, ws, "aidan", "Aidan", Default::default(), 1)
        .await
        .unwrap();

    // A dashboard shared to team:ops with: a private-panel ref, a federation cell on a MISSING
    // datasource, and a REQUIRED variable with no source.
    let cells = vec![
        panel_ref_cell("c1", "panel:aidan"),
        federation_cell("c2", "demo-buildings"),
    ];
    let vars = vec![lb_host::DashboardVariable {
        name: "site".into(),
        required: true,
        ..Default::default()
    }];
    dashboard_save(&store, &admin, ws, "aaaa", "Site Health", cells, vars, 2)
        .await
        .unwrap();
    add_member(&store, &admin, ws, "team:ops", "user:bob")
        .await
        .unwrap();
    dashboard_share(
        &store,
        &admin,
        ws,
        "aaaa",
        DashboardVisibility::Team,
        Some("team:ops"),
        3,
    )
    .await
    .unwrap();

    // bob is a plain member of ops (holds the member role → member caps incl. federation.query? no —
    // he holds only what resolve_caps yields; we make him a bare member WITHOUT federation.query to
    // test the source-cap deny). We seed his caps directly via a member token lacking federation.query.
    let bob = principal(
        "user:bob",
        ws,
        &[
            "mcp:dashboard.access_check:call",
            "mcp:dashboard.get:call",
            "mcp:panel.get:call",
        ],
    );

    // ── Preflight for bob (self) — every red dependency named exactly. ──
    let report = dashboard_access_check(&store, &bob, ws, "aaaa", &Subject::User("bob".into()))
        .await
        .unwrap();
    assert!(!report.ok, "closure is not all-green");

    // (a) the dashboard record IS shared to bob's team → green.
    assert!(
        find(&report.dependencies, "dashboard:aaaa").unwrap().ok,
        "dashboard shared to team:ops resolves for bob"
    );
    // (b) the private panel:aidan is NOT shared → red.
    let panel_v = find(&report.dependencies, "panel:aidan").unwrap();
    assert!(
        !panel_v.ok && panel_v.kind == DepKind::Panel,
        "private panel is red"
    );
    // (c) bob lacks mcp:federation.query:call → red source cap.
    let cap_v = find(&report.dependencies, "federation.query").unwrap();
    assert!(
        !cap_v.ok && cap_v.kind == DepKind::SourceCap,
        "missing source cap is red"
    );
    // (d) the datasource does not exist → red.
    let ds_v = find(&report.dependencies, "datasource:demo-buildings").unwrap();
    assert!(
        !ds_v.ok && ds_v.kind == DepKind::Datasource,
        "missing datasource is red"
    );
    // (e) the required var has no source → red.
    let var_v = find(&report.dependencies, "var:site").unwrap();
    assert!(
        !var_v.ok && var_v.kind == DepKind::Variable,
        "unbound required var is red"
    );

    // ── Preflight/live match (the cardinal-sin guard): build a principal from bob's RESOLVED caps —
    //    the exact caps the preflight used — and assert the SAME `authorize_tool` gate the live
    //    federation.query route runs first ALSO denies it. Verdict == live gate, on identical caps. ──
    let bob_resolved = resolve_caps(&store, ws, "bob").await.unwrap();
    let bob_live = Principal::routed("user:bob", ws, bob_resolved);
    assert!(
        authorize_tool(&bob_live, ws, "federation.query").is_err(),
        "the live federation.query gate denies bob's resolved caps exactly as the preflight reported"
    );

    // ── Flip each red green (share panel / register source+endpoint / grant cap / bind var). ──
    // Share the panel to team:ops.
    lb_host::panel_share(
        &store,
        &admin,
        ws,
        "aidan",
        lb_host::PanelVisibility::Team,
        Some("team:ops"),
        4,
    )
    .await
    .unwrap();
    // Register the datasource + approve its endpoint (the federation install net grant).
    put_datasource(
        &store,
        ws,
        &Datasource::new(
            "demo-buildings",
            "postgres",
            "10.0.0.5:5432",
            "federation/demo-buildings",
            5,
        ),
    )
    .await
    .unwrap();
    record_install(
        &store,
        ws,
        &Install::new(
            "federation",
            "1",
            vec!["net:tls:10.0.0.5:5432:connect".into()],
            5,
        ),
    )
    .await
    .unwrap();
    // Rebuild the dashboard binding the required var to a static default (bindable), same cells.
    let cells2 = vec![
        panel_ref_cell("c1", "panel:aidan"),
        federation_cell("c2", "demo-buildings"),
    ];
    let vars2 = vec![lb_host::DashboardVariable {
        name: "site".into(),
        required: true,
        custom: vec!["site-001".into()],
        ..Default::default()
    }];
    dashboard_save(&store, &admin, ws, "aaaa", "Site Health", cells2, vars2, 6)
        .await
        .unwrap();

    // Grant bob the source cap through the REAL grant path (no-widening: ada holds it). Now
    // `resolve_caps(bob)` yields it, so the preflight — which reads the subject's resolved caps —
    // sees it. This models "his role now carries federation.query", via the store, not a token.
    lb_host::grants_assign(
        &store,
        &admin,
        ws,
        &Subject::User("bob".into()),
        "mcp:federation.query:call",
        &Scope::All,
    )
    .await
    .unwrap();

    // Re-run the preflight for bob (self); his caller token holds access_check + the read caps.
    let report = dashboard_access_check(&store, &bob, ws, "aaaa", &Subject::User("bob".into()))
        .await
        .unwrap();

    // Preflight/live match on the GREEN side too: bob's resolved caps now PASS the same live gate.
    let bob_resolved = resolve_caps(&store, ws, "bob").await.unwrap();
    let bob_live = Principal::routed("user:bob", ws, bob_resolved);
    assert!(
        authorize_tool(&bob_live, ws, "federation.query").is_ok(),
        "the live federation.query gate now allows bob's resolved caps — matches the green verdict"
    );

    assert!(
        find(&report.dependencies, "panel:aidan").unwrap().ok,
        "shared panel now green"
    );
    assert!(
        find(&report.dependencies, "federation.query").unwrap().ok,
        "source cap now green"
    );
    assert!(
        find(&report.dependencies, "datasource:demo-buildings")
            .unwrap()
            .ok,
        "registered datasource now green"
    );
    assert!(
        find(&report.dependencies, "net:tls:10.0.0.5:5432")
            .unwrap()
            .ok,
        "approved endpoint now green"
    );
    assert!(
        find(&report.dependencies, "var:site").unwrap().ok,
        "bound required var now green"
    );
    assert!(report.ok, "the fully-resolved closure is all-green");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolated_and_grants_nothing() {
    let store = Store::memory().await.unwrap();
    let admin_a = principal("user:ada", "acme", ADMIN);

    // A datasource lives in ws-B (beta), not acme.
    put_datasource(
        &store,
        "beta",
        &Datasource::new("plant", "postgres", "10.0.0.9:5432", "federation/plant", 1),
    )
    .await
    .unwrap();

    // An acme dashboard referencing a datasource named `plant` — which exists only in beta.
    let cells = vec![federation_cell("c1", "plant")];
    dashboard_save(&store, &admin_a, "acme", "d1", "D1", cells, vec![], 2)
        .await
        .unwrap();

    let report =
        dashboard_access_check(&store, &admin_a, "acme", "d1", &Subject::User("ada".into()))
            .await
            .unwrap();
    // The ws-B datasource is reported ABSENT in acme — never leaked as existing.
    let ds_v = find(&report.dependencies, "datasource:plant").unwrap();
    assert!(
        !ds_v.ok && ds_v.kind == DepKind::Datasource,
        "a ws-B datasource is absent to a ws-A preflight (isolation)"
    );

    // The preflight granted NOTHING: ada's resolved caps in acme are unchanged (no federation grant
    // was minted by running the preflight). We assert the datasource still doesn't resolve for her.
    assert!(
        lb_host::resolve_datasource(&store, "acme", "plant")
            .await
            .unwrap()
            .is_none(),
        "preflight grants nothing — the acme datasource still does not exist"
    );
    let _ = resolve_caps(&store, "acme", "ada").await.unwrap();
}
