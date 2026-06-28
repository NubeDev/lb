//! The dashboard surface, headless (dashboard scope, build step 2). Proves the mandatory categories
//! against a real store: the CRUD round-trip, capability-deny **per verb**, the **gate-3 non-member
//! deny** (a team-shared dashboard read by a member, refused for a non-member), two-workspace
//! isolation, and seed integrity (the seed writes real, tagged series through the real ingest path).
//!
//! A dashboard is an **asset**, so the sharing model is the shipped S4 three-gate one (`share`/`member`
//! edges, reused via `add_member`/`dashboard_share`) — these tests extend the S4 doc-sharing gate to
//! dashboards. No bus is booted (the verbs are pure store), but the multi-thread flavor is kept for
//! suite uniformity.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    add_member, dashboard_delete, dashboard_get, dashboard_list, dashboard_save, dashboard_share,
    seed_iot_demo, series_find, series_read_range, Cell, DashboardError, DashboardVisibility,
};
use lb_store::Store;
use lb_tags::Facet;
use serde_json::json;

/// A principal `sub` in workspace `ws` holding `caps`.
fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const GET: &str = "mcp:dashboard.get:call";
const LIST: &str = "mcp:dashboard.list:call";
const SAVE: &str = "mcp:dashboard.save:call";
const DELETE: &str = "mcp:dashboard.delete:call";
const SHARE: &str = "mcp:dashboard.share:call";
const ALL: &[&str] = &[GET, LIST, SAVE, DELETE, SHARE];

/// One chart cell bound to `series`.
fn chart_cell(series: &str) -> Cell {
    Cell {
        i: "c1".into(),
        x: 0,
        y: 0,
        w: 4,
        h: 3,
        v: 0,
        widget_type: "chart".into(),
        title: String::new(),
        view: String::new(),
        binding: json!({ "series": series }),
        source: Default::default(),
        action: Default::default(),
        options: json!({}),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn crud_round_trip() {
    let ws = "ws-dash-crud";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    // create
    let d = dashboard_save(
        &store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![chart_cell("cooler.temp")],
        vec![],
        10,
    )
    .await
    .unwrap();
    assert_eq!(d.title, "Ops");
    assert_eq!(d.owner, "user:ada");
    assert_eq!(d.visibility, DashboardVisibility::Private);

    // get reflects it
    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.cells.len(), 1);

    // update (same id) — title + cells change, owner preserved
    dashboard_save(
        &store,
        &ada,
        ws,
        "ops",
        "Ops v2",
        vec![chart_cell("cooler.temp"), chart_cell("fryer.state")],
        vec![],
        20,
    )
    .await
    .unwrap();
    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.title, "Ops v2");
    assert_eq!(got.cells.len(), 2);
    assert_eq!(got.updated_ts, 20);

    // list includes it (summary, no cells)
    let roster = dashboard_list(&store, &ada, ws).await.unwrap();
    assert!(roster.iter().any(|s| s.id == "ops" && s.title == "Ops v2"));

    // delete → list excludes it; get is NotFound
    dashboard_delete(&store, &ada, ws, "ops", 30).await.unwrap();
    let roster = dashboard_list(&store, &ada, ws).await.unwrap();
    assert!(!roster.iter().any(|s| s.id == "ops"));
    assert!(matches!(
        dashboard_get(&store, &ada, ws, "ops").await.unwrap_err(),
        DashboardError::NotFound
    ));

    // re-delete is an idempotent no-op
    dashboard_delete(&store, &ada, ws, "ops", 40).await.unwrap();
}

// widget-config-vars scope, Slice 1: a cell's `title` round-trips through `dashboard.save`/`get` with no
// new verb (additive `#[serde(default)]` on `Cell`). A pre-title cell deserializes with an empty title.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cell_title_round_trips() {
    let ws = "ws-dash-title";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    let mut cell = chart_cell("cooler.temp");
    cell.title = "Web01 CPU".into();
    dashboard_save(&store, &ada, ws, "ops", "Ops", vec![cell], vec![], 10)
        .await
        .unwrap();

    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.cells.len(), 1);
    assert_eq!(got.cells[0].title, "Web01 CPU");

    // A cell saved without a title reads back empty (the derived-label fallback is a UI concern).
    dashboard_save(
        &store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![chart_cell("fryer.state")],
        vec![],
        20,
    )
    .await
    .unwrap();
    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.cells[0].title, "");
}

// widget-config-vars scope, Slice 2: a dashboard's `variables[]` round-trip through `dashboard.save`/
// `get` with no new verb (additive `#[serde(default)]` on `Dashboard`). The per-viewer SELECTION lives
// in the URL — only the DEFINITIONS are durable. A pre-variables dashboard reads back an empty list.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn dashboard_variables_round_trip() {
    use lb_host::DashboardVariable;
    let ws = "ws-dash-vars";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    let host_var = DashboardVariable {
        name: "host".into(),
        label: "Host".into(),
        r#type: "query".into(),
        query: json!({ "tool": "store.query", "args": { "sql": "SELECT name FROM host" } }),
        multi: true,
        include_all: true,
        ..Default::default()
    };
    let step_var = DashboardVariable {
        name: "step".into(),
        r#type: "interval".into(),
        interval: vec!["1m".into(), "5m".into(), "1h".into()],
        ..Default::default()
    };
    dashboard_save(
        &store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![],
        vec![host_var, step_var],
        10,
    )
    .await
    .unwrap();

    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.variables.len(), 2);
    assert_eq!(got.variables[0].name, "host");
    assert_eq!(got.variables[0].r#type, "query");
    assert!(got.variables[0].multi && got.variables[0].include_all);
    assert_eq!(got.variables[1].interval, vec!["1m", "5m", "1h"]);

    // A save without variables reads back an empty list (additive default; no selection stored).
    dashboard_save(&store, &ada, ws, "ops", "Ops", vec![], vec![], 20)
        .await
        .unwrap();
    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert!(got.variables.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_verb_is_denied_without_its_cap() {
    let ws = "ws-dash-deny";
    let store = Store::memory().await.unwrap();
    // Ada owns a dashboard (so get/share have a target); the denied principal holds NO dashboard cap.
    let ada = principal("user:ada", ws, ALL);
    dashboard_save(&store, &ada, ws, "ops", "Ops", vec![], vec![], 1)
        .await
        .unwrap();

    let nobody = principal("user:nobody", ws, &[]);
    assert!(matches!(
        dashboard_get(&store, &nobody, ws, "ops").await.unwrap_err(),
        DashboardError::Denied
    ));
    assert!(matches!(
        dashboard_list(&store, &nobody, ws).await.unwrap_err(),
        DashboardError::Denied
    ));
    assert!(matches!(
        dashboard_save(&store, &nobody, ws, "x", "X", vec![], vec![], 1)
            .await
            .unwrap_err(),
        DashboardError::Denied
    ));
    assert!(matches!(
        dashboard_delete(&store, &nobody, ws, "ops", 1)
            .await
            .unwrap_err(),
        DashboardError::Denied
    ));
    assert!(matches!(
        dashboard_share(
            &store,
            &nobody,
            ws,
            "ops",
            DashboardVisibility::Workspace,
            None,
            1
        )
        .await
        .unwrap_err(),
        DashboardError::Denied
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn team_shared_member_reads_non_member_denied() {
    let ws = "ws-dash-share";
    let store = Store::memory().await.unwrap();
    // Ada owns + admins: she needs `store:doc/*:write` to add a team member (the S4 membership edge
    // is gated there, reused wholesale).
    let ada = principal(
        "user:ada",
        ws,
        &[GET, LIST, SAVE, DELETE, SHARE, "store:doc/*:write"],
    );
    let ben = principal("user:ben", ws, &[GET, LIST]); // team member, read only
    let cleo = principal("user:cleo", ws, &[GET, LIST]); // NOT in the team

    dashboard_save(&store, &ada, ws, "ops", "Ops", vec![], vec![], 1)
        .await
        .unwrap();

    // Private: even a member-cap holder who is not the owner is denied (gate 3).
    assert!(matches!(
        dashboard_get(&store, &ben, ws, "ops").await.unwrap_err(),
        DashboardError::Denied
    ));

    // Share to a team Ben belongs to.
    add_member(&store, &ada, ws, "team:ops", "user:ben")
        .await
        .unwrap();
    dashboard_share(
        &store,
        &ada,
        ws,
        "ops",
        DashboardVisibility::Team,
        Some("team:ops"),
        2,
    )
    .await
    .unwrap();

    // Ben (member) reads it; Cleo (non-member) is DENIED — the gate-3 deny.
    assert_eq!(
        dashboard_get(&store, &ben, ws, "ops").await.unwrap().id,
        "ops"
    );
    assert!(matches!(
        dashboard_get(&store, &cleo, ws, "ops").await.unwrap_err(),
        DashboardError::Denied
    ));

    // The roster is membership-filtered: Ben sees it, Cleo does not.
    assert!(dashboard_list(&store, &ben, ws)
        .await
        .unwrap()
        .iter()
        .any(|s| s.id == "ops"));
    assert!(!dashboard_list(&store, &cleo, ws)
        .await
        .unwrap()
        .iter()
        .any(|s| s.id == "ops"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation() {
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", "ws-a", ALL);
    let ben = principal("user:ben", "ws-b", ALL);

    dashboard_save(&store, &ada, "ws-a", "ops", "Ops A", vec![], vec![], 1)
        .await
        .unwrap();

    // Ben (ws-B) cannot get ws-A's dashboard (a different namespace → not found) and his roster is
    // empty — the workspace wall, structural.
    assert!(matches!(
        dashboard_get(&store, &ben, "ws-b", "ops")
            .await
            .unwrap_err(),
        DashboardError::NotFound
    ));
    assert!(dashboard_list(&store, &ben, "ws-b")
        .await
        .unwrap()
        .is_empty());
    // And a non-owner cannot overwrite the owner's dashboard even within the same workspace.
    let mallory = principal("user:mallory", "ws-a", ALL);
    assert!(matches!(
        dashboard_save(&store, &mallory, "ws-a", "ops", "hijack", vec![], vec![], 2)
            .await
            .unwrap_err(),
        DashboardError::Denied
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn seed_writes_real_tagged_series() {
    let ws = "ws-dash-seed";
    let store = Store::memory().await.unwrap();
    let reader = principal(
        "user:ada",
        ws,
        &["mcp:series.read:call", "mcp:series.find:call"],
    );

    let report = seed_iot_demo(&store, ws, 0).await.unwrap();
    assert_eq!(report.series, vec!["cooler.temp", "fryer.state"]);
    assert!(report.samples_committed >= 48);

    // series.read returns the seeded cooler samples (committed, not faked).
    let rows = series_read_range(&store, &reader, ws, "cooler.temp", None, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 24);

    // series.find resolves the tagged series via the faceted tag query.
    let hits = series_find(
        &store,
        &reader,
        ws,
        &[Facet::exact("kind", json!("temperature"))],
    )
    .await
    .unwrap();
    assert_eq!(hits, vec!["series:cooler.temp"]);
}
