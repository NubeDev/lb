//! The library-panels surface, headless (library-panels scope, the "Testing plan"). Proves the
//! mandatory categories against a real store/node with real seeded records (rule 9): the CRUD
//! round-trip, capability-deny **per verb**, the **gate-3 non-member deny**, two-workspace isolation
//! (incl. the **cross-ws `panel_ref` no-hydrate** headline), the **"sharing never widens data access"**
//! headline (definition readable, data re-checked under the viewer), inline/ref **coexistence** +
//! **propagation**, the ref-is-authoritative (echoed-spec-ignored) rule, dangling-ref **placeholder**,
//! and **delete-safety**.
//!
//! A panel is an **asset**, so the sharing model is the shipped S4 three-gate one — identical to the
//! dashboard/nav tests, cloned one level down.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    add_member, call_tool, dashboard_get, panel_delete, panel_get, panel_list, panel_save,
    panel_share, panel_usage, Cell, CellSource, CellTarget, DashboardError, Node, PanelError,
    PanelSpec, PanelVisibility,
};
use lb_store::Store;
use serde_json::{json, Value};

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

const GET: &str = "mcp:panel.get:call";
const LIST: &str = "mcp:panel.list:call";
const SAVE: &str = "mcp:panel.save:call";
const DELETE: &str = "mcp:panel.delete:call";
const SHARE: &str = "mcp:panel.share:call";
const USAGE: &str = "mcp:panel.usage:call";
const ALL: &[&str] = &[GET, LIST, SAVE, DELETE, SHARE, USAGE];

const DASH_SAVE: &str = "mcp:dashboard.save:call";
const DASH_GET: &str = "mcp:dashboard.get:call";
const VIZ_QUERY: &str = "mcp:viz.query:call";

// --- constructors -------------------------------------------------------------------------------

/// A timeseries spec reading one series through a `series.read` target — the data leash the "never
/// widens" test exercises.
fn series_spec() -> PanelSpec {
    PanelSpec {
        v: 3,
        widget_type: "chart".into(),
        title: "Cooler temp".into(),
        view: "timeseries".into(),
        sources: vec![CellTarget {
            ref_id: "A".into(),
            tool: "series.read".into(),
            args: json!({ "series": "cooler.temp" }),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// A minimal inline v3 cell (no `panel_ref`).
fn inline_cell(i: &str, view: &str) -> Cell {
    Cell {
        i: i.into(),
        x: 0,
        y: 0,
        w: 6,
        h: 4,
        v: 3,
        widget_type: "chart".into(),
        title: format!("inline {i}"),
        view: view.into(),
        binding: Value::Null,
        source: CellSource::default(),
        action: Default::default(),
        options: Value::Null,
        description: String::new(),
        sources: vec![],
        transformations: vec![],
        field_config: Value::Null,
        plugin_version: String::new(),
        panel_ref: String::new(),
        panel_vars: Value::Null,
        panel_missing: false,
    }
}

/// A ref cell pointing at `panel:{id}` — layout + the ref + a bogus echoed spec that MUST be ignored.
fn ref_cell(i: &str, panel_ref: &str) -> Cell {
    Cell {
        panel_ref: panel_ref.into(),
        // A deliberately WRONG echoed spec: proves the ref is authoritative (stripped on save,
        // re-hydrated from the panel record on read).
        view: "STALE".into(),
        title: String::new(),
        ..inline_cell(i, "STALE")
    }
}

// --- CRUD ---------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn crud_round_trip() {
    let ws = "ws-panel-crud";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    let p = panel_save(&store, &ada, ws, "cooler", "Cooler", series_spec(), 10)
        .await
        .unwrap();
    assert_eq!(p.title, "Cooler");
    assert_eq!(p.owner, "user:ada");
    assert_eq!(p.visibility, PanelVisibility::Private);
    assert_eq!(p.spec.view, "timeseries");

    // get reflects it (full spec)
    let got = panel_get(&store, &ada, ws, "cooler").await.unwrap();
    assert_eq!(got.spec.sources[0].tool, "series.read");

    // update (same id) — LWW, owner preserved
    let mut spec2 = series_spec();
    spec2.title = "Cooler v2".into();
    panel_save(&store, &ada, ws, "cooler", "Cooler v2", spec2, 20)
        .await
        .unwrap();
    let got = panel_get(&store, &ada, ws, "cooler").await.unwrap();
    assert_eq!(got.title, "Cooler v2");
    assert_eq!(got.updated_ts, 20);

    // list = cheap summary (view carried, no spec)
    let roster = panel_list(&store, &ada, ws).await.unwrap();
    let row = roster.iter().find(|s| s.id == "cooler").unwrap();
    assert_eq!(row.view, "timeseries");

    // delete → gone; get NotFound; re-delete idempotent
    panel_delete(&store, &ada, ws, "cooler", false, 30)
        .await
        .unwrap();
    assert!(matches!(
        panel_get(&store, &ada, ws, "cooler").await.unwrap_err(),
        PanelError::NotFound
    ));
    panel_delete(&store, &ada, ws, "cooler", false, 40)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn over_cap_spec_rejected() {
    let ws = "ws-panel-bounds";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);
    let mut spec = series_spec();
    // Over the transform cap → rejected (the host is the boundary, same as dashboard cells).
    spec.transformations = (0..64).map(|_| json!({ "id": "x" })).collect();
    assert!(matches!(
        panel_save(&store, &ada, ws, "big", "Big", spec, 1)
            .await
            .unwrap_err(),
        PanelError::BadInput(_)
    ));
}

// --- mandatory: capability deny per verb --------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_verb_is_denied_without_its_cap() {
    let ws = "ws-panel-deny";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);
    panel_save(&store, &ada, ws, "p", "P", series_spec(), 1)
        .await
        .unwrap();

    let nobody = principal("user:nobody", ws, &[]);
    assert!(matches!(
        panel_get(&store, &nobody, ws, "p").await.unwrap_err(),
        PanelError::Denied
    ));
    assert!(matches!(
        panel_list(&store, &nobody, ws).await.unwrap_err(),
        PanelError::Denied
    ));
    assert!(matches!(
        panel_save(&store, &nobody, ws, "x", "X", series_spec(), 1)
            .await
            .unwrap_err(),
        PanelError::Denied
    ));
    assert!(matches!(
        panel_delete(&store, &nobody, ws, "p", false, 1)
            .await
            .unwrap_err(),
        PanelError::Denied
    ));
    assert!(matches!(
        panel_share(
            &store,
            &nobody,
            ws,
            "p",
            PanelVisibility::Workspace,
            None,
            1
        )
        .await
        .unwrap_err(),
        PanelError::Denied
    ));
    assert!(matches!(
        panel_usage(&store, &nobody, ws, "p").await.unwrap_err(),
        PanelError::Denied
    ));
}

// --- mandatory: workspace isolation -------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation() {
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", "ws-a", ALL);
    let ben = principal("user:ben", "ws-b", ALL);

    panel_save(&store, &ada, "ws-a", "p", "P", series_spec(), 1)
        .await
        .unwrap();

    // Ben (ws-B) cannot get/list ws-A's panel — the wall.
    assert!(matches!(
        panel_get(&store, &ben, "ws-b", "p").await.unwrap_err(),
        PanelError::NotFound
    ));
    assert!(panel_list(&store, &ben, "ws-b").await.unwrap().is_empty());

    // A non-owner cannot overwrite the owner's panel even in the same workspace.
    let mallory = principal("user:mallory", "ws-a", ALL);
    assert!(matches!(
        panel_save(&store, &mallory, "ws-a", "p", "hijack", series_spec(), 2)
            .await
            .unwrap_err(),
        PanelError::Denied
    ));
}

// --- mandatory: gate-3 team-shared deny (non-member) --------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn team_shared_member_reads_non_member_denied() {
    let ws = "ws-panel-share";
    let node = Node::boot().await.unwrap();
    let store = &node.store;
    let ada = principal("user:ada", ws, &[GET, SAVE, SHARE, "store:doc/*:write"]);
    let ben = principal("user:ben", ws, &[GET]); // team member
    let cleo = principal("user:cleo", ws, &[GET]); // NOT in the team

    panel_save(store, &ada, ws, "p", "P", series_spec(), 1)
        .await
        .unwrap();

    // Private: a non-owner member is denied gate 3.
    assert!(matches!(
        panel_get(store, &ben, ws, "p").await.unwrap_err(),
        PanelError::Denied
    ));

    add_member(store, &ada, ws, "team:ops", "user:ben")
        .await
        .unwrap();
    panel_share(
        store,
        &ada,
        ws,
        "p",
        PanelVisibility::Team,
        Some("team:ops"),
        2,
    )
    .await
    .unwrap();

    assert_eq!(panel_get(store, &ben, ws, "p").await.unwrap().id, "p");
    assert!(matches!(
        panel_get(store, &cleo, ws, "p").await.unwrap_err(),
        PanelError::Denied
    ));
}

// --- HEADLINE: sharing never widens DATA access -------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn sharing_never_widens_data_access() {
    let ws = "ws-panel-lens";
    let node = Arc::new(Node::boot().await.unwrap());
    let store = &node.store;

    // Ada publishes a workspace-visible panel whose source needs `series.read`.
    let ada = principal("user:ada", ws, &[SAVE, SHARE, GET]);
    panel_save(store, &ada, ws, "cooler", "Cooler", series_spec(), 1)
        .await
        .unwrap();
    panel_share(
        store,
        &ada,
        ws,
        "cooler",
        PanelVisibility::Workspace,
        None,
        2,
    )
    .await
    .unwrap();

    // Ben reads the DEFINITION (panel.get, workspace-visible) but holds NO `series.read`. He holds
    // viz.query, so the render path runs — but the target is denied → an honest EMPTY frame, never a
    // leak of the shared data.
    let ben = principal("user:ben", ws, &[GET, VIZ_QUERY]);
    let def = panel_get(store, &ben, ws, "cooler").await.unwrap();
    assert_eq!(
        def.spec.sources[0].tool, "series.read",
        "definition readable"
    );

    // viz.query over the panel's spec → frames present, rows EMPTY (denied target, no fabricated data).
    let panel_arg = json!({ "sources": def.spec.sources });
    let out = call_tool(
        &node,
        &ben,
        ws,
        "viz.query",
        &json!({ "panel": panel_arg, "now": 3 }).to_string(),
    )
    .await
    .expect("viz.query verb authorized (Ben holds it)");
    let out: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        out["rows"].as_array().map(|a| a.len()),
        Some(0),
        "denied target → no data, not a leak"
    );

    // AND the data tool itself is denied to Ben directly — the panel granted nothing.
    let err = call_tool(
        &node,
        &ben,
        ws,
        "series.read",
        &json!({ "series": "cooler.temp" }).to_string(),
    )
    .await
    .expect_err("series.read denied — sharing the panel widened no data access");
    assert!(format!("{err:?}").to_lowercase().contains("deni"));
}

// --- hydration: inline/ref coexistence + propagation + echoed-spec-ignored ----------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ref_hydrates_coexists_propagates_and_ignores_echoed_spec() {
    let ws = "ws-panel-hydrate";
    let store = Store::memory().await.unwrap();
    let store = &store;
    let ada = principal("user:ada", ws, &[SAVE, GET, DASH_SAVE, DASH_GET]);

    // A panel + a dashboard with an inline cell AND a ref cell (coexistence).
    panel_save(store, &ada, ws, "cooler", "Cooler", series_spec(), 1)
        .await
        .unwrap();
    lb_host::dashboard_save(
        store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![inline_cell("c1", "stat"), ref_cell("c2", "panel:cooler")],
        vec![],
        2,
    )
    .await
    .unwrap();

    // dashboard.get hydrates the ref cell from the panel (NOT the stale echoed "STALE" spec), leaving
    // the inline cell untouched. The `panel_ref` marker is kept.
    let d = dashboard_get(store, &ada, ws, "ops").await.unwrap();
    let inline = d.cells.iter().find(|c| c.i == "c1").unwrap();
    let refc = d.cells.iter().find(|c| c.i == "c2").unwrap();
    assert_eq!(inline.view, "stat", "inline cell unchanged");
    assert!(inline.panel_ref.is_empty(), "inline never grows a ref");
    assert_eq!(
        refc.view, "timeseries",
        "ref hydrated from the panel record"
    );
    assert_eq!(refc.panel_ref, "panel:cooler", "marker kept");
    assert_ne!(
        refc.view, "STALE",
        "echoed spec ignored (ref authoritative)"
    );

    // Propagation: edit the panel once → the dashboard reflects it on next get (edit-once-reuse).
    let mut spec2 = series_spec();
    spec2.view = "gauge".into();
    panel_save(store, &ada, ws, "cooler", "Cooler", spec2, 3)
        .await
        .unwrap();
    let d = dashboard_get(store, &ada, ws, "ops").await.unwrap();
    let refc = d.cells.iter().find(|c| c.i == "c2").unwrap();
    assert_eq!(refc.view, "gauge", "panel edit propagated to the ref cell");

    // Unlink (client copies spec inline, drops the ref): the inline copy stops tracking the panel.
    lb_host::dashboard_save(
        store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![inline_cell("c1", "stat"), inline_cell("c2", "gauge")],
        vec![],
        4,
    )
    .await
    .unwrap();
    let mut spec3 = series_spec();
    spec3.view = "table".into();
    panel_save(store, &ada, ws, "cooler", "Cooler", spec3, 5)
        .await
        .unwrap();
    let d = dashboard_get(store, &ada, ws, "ops").await.unwrap();
    let unlinked = d.cells.iter().find(|c| c.i == "c2").unwrap();
    assert_eq!(unlinked.view, "gauge", "unlinked copy no longer propagates");
}

// --- HEADLINE isolation: cross-ws ref rejected at save + dangling ref → placeholder --------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cross_ws_ref_rejected_and_dangling_placeholders() {
    let store = Store::memory().await.unwrap();
    // Panel only exists in ws-A.
    let ada = principal("user:ada", "ws-a", &[SAVE, GET]);
    panel_save(&store, &ada, "ws-a", "cooler", "Cooler", series_spec(), 1)
        .await
        .unwrap();

    // ws-B dashboard referencing `panel:cooler` (which is NOT in ws-B) → save REJECTED loudly (the
    // cross-ws ref never hydrates because it never persists — validate at write).
    let ben = principal("user:ben", "ws-b", &[DASH_SAVE, DASH_GET, GET]);
    let err = lb_host::dashboard_save(
        &store,
        &ben,
        "ws-b",
        "b",
        "B",
        vec![ref_cell("c1", "panel:cooler")],
        vec![],
        2,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, DashboardError::BadInput(_)),
        "cross-ws ref rejected: {err:?}"
    );

    // Dangling in-workspace: Ada references her panel, then force-deletes it → the cell hydrates to
    // the honest placeholder (panel_missing), never a crash or a leaked spec.
    let ada = principal(
        "user:ada",
        "ws-a",
        &[SAVE, GET, DELETE, USAGE, DASH_SAVE, DASH_GET],
    );
    lb_host::dashboard_save(
        &store,
        &ada,
        "ws-a",
        "ops",
        "Ops",
        vec![ref_cell("c1", "panel:cooler")],
        vec![],
        3,
    )
    .await
    .unwrap();
    panel_delete(&store, &ada, "ws-a", "cooler", true, 4)
        .await
        .unwrap();
    let d = dashboard_get(&store, &ada, "ws-a", "ops").await.unwrap();
    let cell = &d.cells[0];
    assert!(cell.panel_missing, "dangling ref → placeholder");
    assert_eq!(cell.panel_ref, "panel:cooler", "marker kept for relink");
    assert!(cell.sources.is_empty(), "no spec leaked on a missing panel");
}

// --- delete-safety ------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_refused_while_in_use_unless_forced() {
    let ws = "ws-panel-del";
    let store = Store::memory().await.unwrap();
    let store = &store;
    let ada = principal(
        "user:ada",
        ws,
        &[SAVE, GET, DELETE, USAGE, DASH_SAVE, DASH_GET],
    );
    panel_save(&store, &ada, ws, "cooler", "Cooler", series_spec(), 1)
        .await
        .unwrap();
    lb_host::dashboard_save(
        &store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![ref_cell("c1", "panel:cooler")],
        vec![],
        2,
    )
    .await
    .unwrap();

    // usage reports the referencing dashboard.
    let usage = panel_usage(&store, &ada, ws, "cooler").await.unwrap();
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].dashboard, "ops");
    assert_eq!(usage[0].cells, 1);

    // delete-in-use refused with the usage list.
    match panel_delete(&store, &ada, ws, "cooler", false, 3).await {
        Err(PanelError::InUse(rows)) => assert_eq!(rows[0].dashboard, "ops"),
        other => panic!("expected InUse, got {other:?}"),
    }

    // force → tombstone; the ref cell now hydrates to the placeholder.
    panel_delete(&store, &ada, ws, "cooler", true, 4)
        .await
        .unwrap();
    let d = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert!(d.cells[0].panel_missing);

    // re-saving the panel un-hides it (dashboard tombstone semantics) — the ref re-hydrates.
    panel_save(&store, &ada, ws, "cooler", "Cooler", series_spec(), 5)
        .await
        .unwrap();
    let d = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert!(
        !d.cells[0].panel_missing,
        "re-created panel re-hydrates the ref"
    );
    assert_eq!(d.cells[0].view, "timeseries");
}

// --- REGRESSION: `dashboard.save` returns hydrated ref cells -------------------------------------
//
// The headline library-panels invariant: every render host (grid, editor, standalone) sees plain
// hydrated v3 cells, ref-aware ONLY in the editor's link/unlink. The client `setCurrent`s the save's
// return after every dashboard edit (drag/resize/add/remove/duplicate), so `dashboard_save` MUST
// return ref cells HYDRATED — same as `dashboard.get` — not the stripped layout+ref form that gets
// persisted. Otherwise any edit to a dashboard containing a ref cell blanks those cells ("Unsupported
// widget") until the next reload. The persisted record stays stripped (the ref is authoritative); only
// the RETURNED value is re-hydrated for display.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn dashboard_save_returns_hydrated_ref_cells() {
    let ws = "ws-save-hydrate";
    let store = Store::memory().await.unwrap();
    let store = &store;
    let ada = principal("user:ada", ws, &[SAVE, GET, DASH_SAVE, DASH_GET]);

    panel_save(store, &ada, ws, "cooler", "Cooler", series_spec(), 1)
        .await
        .unwrap();

    // A dashboard with one ref cell. The cell carries a deliberately-wrong echoed spec to prove the
    // returned value is the panel's spec (re-hydrated), not the client's echoed copy.
    let saved = lb_host::dashboard_save(
        store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![ref_cell("c1", "panel:cooler")],
        vec![],
        2,
    )
    .await
    .unwrap();

    // The SAVE's return must carry the hydrated spec — view from the panel, NOT the echoed "STALE",
    // NOT the empty-string stripped form. This is the exact gap that surfaced as "Unsupported widget"
    // on every dashboard edit (drag/resize/duplicate) before the fix.
    let cell = &saved.cells[0];
    assert_eq!(cell.panel_ref, "panel:cooler", "marker kept");
    assert!(!cell.panel_missing, "panel resolves → not the placeholder");
    assert_eq!(
        cell.view, "timeseries",
        "save returns the hydrated spec (the panel record's view)"
    );
    assert_eq!(
        cell.widget_type, "chart",
        "save returns the hydrated widget_type"
    );
    assert_eq!(
        cell.sources.len(),
        1,
        "save returns the hydrated sources (one series.read target)"
    );
    assert_ne!(
        cell.view, "STALE",
        "echoed spec ignored — the ref is authoritative"
    );

    // The persisted record stays STRIPPED (layout + ref + overrides only): prove the round-trip via a
    // fresh `dashboard.get` re-hydrates from the panel, not from anything the save echoed back.
    let d = dashboard_get(store, &ada, ws, "ops").await.unwrap();
    assert_eq!(d.cells[0].view, "timeseries");
    assert_eq!(d.cells[0].panel_ref, "panel:cooler");

    // And a second save (the edit-after-edit case — e.g. a duplicate) still returns hydrated cells.
    let saved2 = lb_host::dashboard_save(
        store,
        &ada,
        ws,
        "ops",
        "Ops",
        saved.cells.clone(), // the client re-sends what it last received (the hydrated form)
        vec![],
        3,
    )
    .await
    .unwrap();
    assert_eq!(
        saved2.cells[0].view, "timeseries",
        "a re-save still returns hydrated ref cells"
    );
    assert_eq!(saved2.cells[0].panel_ref, "panel:cooler");
}
