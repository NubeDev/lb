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
    add_member, dashboard_delete, dashboard_get, dashboard_list, dashboard_save,
    dashboard_save_meta, dashboard_share, seed_iot_demo, series_find, series_read_range, Cell,
    CellSource, CellTarget, DashboardError, DashboardToolbar as Toolbar, DashboardVisibility,
    DASHBOARD_MAX_OVERRIDES, DASHBOARD_MAX_TRANSFORMS,
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
        constraint: None,
        run_id: None,
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

/// One chart cell bound to `series` (a v1 cell — all v2/v3 fields defaulted/absent).
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
        description: String::new(),
        sources: Vec::new(),
        transformations: Vec::new(),
        field_config: json!(null),
        plugin_version: String::new(),
        panel_ref: String::new(),
        panel_vars: json!(null),
        panel_missing: false,
        ..Default::default()
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

// dashboard page-settings: `description`/`icon`/`color` set via `dashboard_save_meta` round-trip through
// get + list, and are PRESERVED across a plain `dashboard_save` (a layout/variable save must never blank
// the page chrome — the same preserve-on-omit discipline `visibility` uses).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn page_settings_round_trip_and_preserve() {
    let ws = "ws-dash-settings";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    // Create with page settings.
    dashboard_save_meta(
        &store,
        &ada,
        ws,
        "ops",
        "Ops",
        Some("Fleet health at a glance".into()),
        Some("activity".into()),
        Some("#3b82f6".into()),
        None,
        // Opt the date-select + share controls into the header (toolbar-settings); refresh stays hidden.
        Some(Toolbar {
            date_select: true,
            refresh_rate: false,
            share: true,
        }),
        vec![chart_cell("cooler.temp")],
        vec![],
        10,
    )
    .await
    .unwrap();

    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.description, "Fleet health at a glance");
    assert_eq!(got.icon, "activity");
    assert_eq!(got.color, "#3b82f6");
    assert!(got.toolbar.date_select && got.toolbar.share && !got.toolbar.refresh_rate);

    // The cheap summary carries icon + colour (roster paints them without a full get).
    let roster = dashboard_list(&store, &ada, ws).await.unwrap();
    let row = roster.iter().find(|s| s.id == "ops").unwrap();
    assert_eq!(row.icon, "activity");
    assert_eq!(row.color, "#3b82f6");

    // A plain layout save (the wrapper: no settings args) PRESERVES the page chrome.
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
    assert_eq!(got.description, "Fleet health at a glance");
    assert_eq!(got.icon, "activity");
    assert_eq!(got.color, "#3b82f6");
    // Toolbar flags are page chrome too — a plain layout save preserves them.
    assert!(got.toolbar.date_select && got.toolbar.share && !got.toolbar.refresh_rate);

    // Setting one field via meta preserves the others (Some on icon, None on the rest — incl. toolbar).
    dashboard_save_meta(
        &store,
        &ada,
        ws,
        "ops",
        "Ops v2",
        None,
        Some("gauge".into()),
        None,
        None,
        None,
        got.cells.clone(),
        vec![],
        30,
    )
    .await
    .unwrap();
    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.icon, "gauge");
    assert_eq!(got.description, "Fleet health at a glance");
    assert_eq!(got.color, "#3b82f6");
    // `None` toolbar on the meta save preserved the opted-in flags (never re-hidden by a partial edit).
    assert!(got.toolbar.date_select && got.toolbar.share && !got.toolbar.refresh_rate);
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

// dashboard.delete's admin override: a non-owner holding the base DELETE cap alone stays denied
// (owner-only, like save/share); granting `dashboard.delete_any` too lets them tombstone someone
// else's dashboard. Regression for the UI bug where an admin's delete confirm silently no-op'd —
// the roster showed the delete affordance for every dashboard (gated on ANY admin cap) but the host
// only ever checked ownership, so a non-owner admin's click ran the confirm dialog and then hit an
// opaque Denied with no visible feedback.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_any_cap_lets_a_non_owner_admin_delete() {
    let ws = "ws-dash-delete-any";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);
    dashboard_save(&store, &ada, ws, "ops", "Ops", vec![], vec![], 1)
        .await
        .unwrap();

    // A non-owner with the plain DELETE cap (no admin override) is still denied.
    let admin_without_override = principal("user:admin", ws, &[GET, LIST, DELETE]);
    assert!(matches!(
        dashboard_delete(&store, &admin_without_override, ws, "ops", 2)
            .await
            .unwrap_err(),
        DashboardError::Denied
    ));
    // Still there — the denied attempt did not tombstone it.
    let roster = dashboard_list(&store, &ada, ws).await.unwrap();
    assert!(roster.iter().any(|s| s.id == "ops"));

    // The same principal, now also holding `dashboard.delete_any`, succeeds.
    let admin_with_override = principal(
        "user:admin",
        ws,
        &[GET, LIST, DELETE, "mcp:dashboard.delete_any:call"],
    );
    dashboard_delete(&store, &admin_with_override, ws, "ops", 3)
        .await
        .unwrap();
    let roster = dashboard_list(&store, &ada, ws).await.unwrap();
    assert!(!roster.iter().any(|s| s.id == "ops"));
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

// ---------------------------------------------------------------------------------------------------
// viz panel-model scope (Phase 1): the additive v3 cell shape round-trips through dashboard.save/get,
// a v1/v2 cell still loads unchanged, schemaVersion is pinned at save, and the record bounds reject an
// over-cap fieldConfig/transforms list (the host is the boundary, not the editor).
// ---------------------------------------------------------------------------------------------------

/// A full v3 timeseries cell: targets[], fieldConfig (unit/decimals/min/max/thresholds), per-view
/// options, transformations config — every additive field set.
fn v3_timeseries_cell() -> Cell {
    Cell {
        i: "p1".into(),
        x: 0,
        y: 0,
        w: 8,
        h: 4,
        v: 3,
        widget_type: "chart".into(),
        title: "Cooler °C".into(),
        view: "timeseries".into(),
        binding: json!({ "series": "" }),
        source: Default::default(),
        action: Default::default(),
        options: json!({ "legend": { "showLegend": true, "displayMode": "table", "placement": "bottom", "calcs": ["mean", "max"] }, "tooltip": { "mode": "single", "sort": "none" } }),
        description: "panel desc".into(),
        sources: vec![CellTarget {
            ref_id: "A".into(),
            datasource: json!({ "type": "surreal" }),
            tool: "store.query".into(),
            args: json!({ "sql": "SELECT value FROM reading" }),
            hide: false,
        }],
        transformations: vec![json!({ "id": "reduce", "options": { "reducers": ["last"] } })],
        field_config: json!({
            "defaults": {
                "unit": "celsius",
                "decimals": 1,
                "min": 0,
                "max": 50,
                "thresholds": { "mode": "absolute", "steps": [ { "value": null, "color": "green" }, { "value": 5, "color": "red" } ] }
            },
            "overrides": []
        }),
        plugin_version: "lb-viz@1".into(),
        panel_ref: String::new(),
        panel_vars: json!(null),
        panel_missing: false,
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn v3_cell_round_trips() {
    let ws = "ws-dash-v3";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    let cell = v3_timeseries_cell();
    let saved = dashboard_save(
        &store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![cell.clone()],
        vec![],
        10,
    )
    .await
    .unwrap();
    // schemaVersion is pinned to the panel-model document version at save.
    assert_eq!(saved.schema_version, 3);

    // get re-reads every additive v3 field (the round-trip the editor's add≡edit relies on). The store
    // normalizes an explicit JSON `null` away (the base threshold step's `value:null` ⇒ key absent),
    // which the UI treats as -∞ — so we assert FIELD-LEVEL fidelity, not byte equality, for the parts
    // that carry a null. Everything else is identical.
    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    let back = &got.cells[0];
    assert_eq!(back.v, 3);
    assert_eq!(back.view, "timeseries");
    assert_eq!(back.title, "Cooler °C");
    assert_eq!(back.description, "panel desc");
    assert_eq!(back.plugin_version, "lb-viz@1");
    assert_eq!(back.options, cell.options, "per-view options round-trip");
    assert_eq!(back.sources, cell.sources, "targets round-trip");
    assert_eq!(back.sources[0].ref_id, "A");
    assert_eq!(
        back.transformations, cell.transformations,
        "transform config round-trips"
    );
    // fieldConfig: unit/decimals/min/max + the threshold step colors survive (the base step's null
    // value is dropped by the store, recognized as -∞ by the UI).
    let fc = &back.field_config["defaults"];
    assert_eq!(fc["unit"], json!("celsius"));
    assert_eq!(fc["decimals"], json!(1));
    assert_eq!(fc["min"], json!(0));
    assert_eq!(fc["max"], json!(50));
    let steps = fc["thresholds"]["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0]["color"], json!("green"));
    assert_eq!(steps[1]["color"], json!("red"));
    assert_eq!(steps[1]["value"], json!(5));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn v1_and_v2_cells_still_load_after_v3() {
    let ws = "ws-dash-compat";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    // A v1 series cell (no view/source) and a v2 chart+store.query cell both save + re-read unchanged —
    // the v3 fields stay absent/defaulted, never injected.
    let v1 = chart_cell("cooler.temp");
    let mut v2 = chart_cell("");
    v2.i = "c2".into();
    v2.v = 2;
    v2.view = "chart".into();
    v2.source = CellSource {
        tool: "store.query".into(),
        args: json!({ "sql": "SELECT 1" }),
    };

    dashboard_save(
        &store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![v1.clone(), v2.clone()],
        vec![],
        10,
    )
    .await
    .unwrap();
    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.cells[0], v1, "v1 cell round-trips byte-identical");
    assert_eq!(got.cells[1], v2, "v2 cell round-trips byte-identical");
    // The additive v3 fields are absent/defaulted on both (no spurious data).
    assert!(got.cells[0].sources.is_empty());
    assert_eq!(got.cells[0].field_config, json!(null));
    assert!(got.cells[1].sources.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn over_cap_v3_record_is_rejected() {
    let ws = "ws-dash-bounds";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    // Too many transformations → rejected (the host is the boundary, not the editor).
    let mut cell = v3_timeseries_cell();
    cell.transformations = (0..(DASHBOARD_MAX_TRANSFORMS + 1))
        .map(|_| json!({ "id": "reduce" }))
        .collect();
    assert!(matches!(
        dashboard_save(&store, &ada, ws, "ops", "Ops", vec![cell], vec![], 10)
            .await
            .unwrap_err(),
        DashboardError::BadInput(_)
    ));

    // Too many fieldConfig overrides → rejected.
    let mut cell = v3_timeseries_cell();
    let overrides: Vec<_> = (0..(DASHBOARD_MAX_OVERRIDES + 1))
        .map(|i| json!({ "matcher": { "id": "byName", "options": format!("f{i}") }, "properties": [] }))
        .collect();
    cell.field_config = json!({ "defaults": {}, "overrides": overrides });
    assert!(matches!(
        dashboard_save(&store, &ada, ws, "ops2", "Ops", vec![cell], vec![], 10)
            .await
            .unwrap_err(),
        DashboardError::BadInput(_)
    ));

    // A within-cap v3 cell is accepted (the bound is a ceiling, not a block).
    let cell = v3_timeseries_cell();
    assert!(
        dashboard_save(&store, &ada, ws, "ops3", "Ops", vec![cell], vec![], 10)
            .await
            .is_ok()
    );
}
