//! Grafana JSON import/export over the real gateway (viz import-export scope, Phase 4) — the mapper +
//! the two verbs end to end. No fakes: a real gateway, a real store, a real datasource record seeded
//! into the real store; a genuine Grafana export `.json` is a FIXTURE, not a fake backend. Covers the
//! scope's mandatory tests: round-trip fidelity, schemaVersion migration, datasource remap + the
//! two-session **workspace isolation** wall, capability deny, honest degradation, and the passthrough
//! bound.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_host::{Datasource, Node, Qos, Role as NodeRole};
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt;

/// Import needs both caps (write); export is the read cap.
const IMPORT_CAPS: &[&str] = &[
    "mcp:dashboard.import:call",
    "mcp:dashboard.save:call",
    "mcp:dashboard.get:call",
    "mcp:dashboard.export:call",
    "mcp:datasource.list:call",
];

/// A realistic pre-v33 Grafana export: `__inputs`, a string→ref datasource via `${DS_PROM}`, a legacy
/// `graph` panel + a modern `timeseries`, an unsupported `heatmap`, a template var, and an unknown
/// panel field to prove passthrough round-trips.
fn grafana_export() -> Value {
    json!({
        "schemaVersion": 30,
        "title": "Imported Ops",
        "__inputs": [{"name": "DS_PROM", "type": "datasource", "pluginId": "prometheus"}],
        "__requires": [{"type": "grafana", "id": "grafana", "version": "9.0.0"}],
        "templating": {"list": [
            {"name": "instance", "type": "query", "datasource": "${DS_PROM}", "query": "up"}
        ]},
        "panels": [
            {"id": 1, "type": "graph", "title": "CPU",
             "gridPos": {"x": 0, "y": 0, "w": 12, "h": 8},
             "datasource": "${DS_PROM}",
             "targets": [{"refId": "A", "expr": "rate(cpu[5m])", "datasource": "${DS_PROM}"}],
             "fieldConfig": {"defaults": {"unit": "percent"}},
             "customPluginField": {"keep": true}},
            {"id": 2, "type": "heatmap", "title": "Heat",
             "gridPos": {"x": 12, "y": 0, "w": 12, "h": 8}}
        ]
    })
}

/// A pdnsw-IAQ-shaped export exercising the fidelity slice end to end: 24-col `gridPos`, `graph` panels
/// carrying real `$__time`/`$__timeFilter` Timescale SQL, a plugin `stat`, a real `text` note, and three
/// decorative placeholders that must DROP (an empty logo `text`, a `dashlist`, a Grafana default
/// `metric_table` banner), plus a dashboard-level annotation plane + `refresh` + `graphTooltip`.
fn grafana_iaq_export() -> Value {
    let ds = json!({ "type": "postgres", "uid": "pdnsw-uid" });
    let chart = |id: u64, title: &str, x: u64| {
        json!({
            "id": id, "type": "graph", "title": title,
            "gridPos": {"x": x, "y": 1, "w": 8, "h": 8},
            "datasource": ds,
            "targets": [{"refId": "A", "datasource": ds,
                "rawSql": "SELECT $__time(histories.timestamp), value FROM histories WHERE $__timeFilter(histories.timestamp) ORDER BY histories.timestamp"}]
        })
    };
    json!({
        "schemaVersion": 30,
        "title": "Indoor Air Quality",
        "timezone": "Australia/Sydney",
        "refresh": "30s",
        "graphTooltip": 1,
        "annotations": {"list": [{"name": "Annotations & Alerts", "datasource": {"uid": "-- Grafana --"}}]},
        "panels": [
            {"id": 2, "type": "row", "title": "Level 2 - Monitoring", "gridPos": {"x": 0, "y": 0, "w": 24, "h": 1}},
            chart(20, "L2 Temperature", 0),
            chart(21, "L2 CO2", 8),
            chart(22, "L2 Humidity", 16),
            {"id": 25, "type": "stat", "title": "TVOC",
             "gridPos": {"x": 0, "y": 9, "w": 8, "h": 4}, "datasource": ds,
             "targets": [{"refId": "A", "datasource": ds,
                "rawSql": "SELECT value FROM histories WHERE point_uuid='p1' ORDER BY timestamp DESC LIMIT 1"}]},
            {"id": 30, "type": "text", "title": "Notes",
             "gridPos": {"x": 0, "y": 13, "w": 24, "h": 2},
             "options": {"mode": "markdown", "content": "# Level 2\nMonitoring notes"}},
            // --- the three decorative placeholders that must DROP ---
            {"id": 31, "type": "text", "title": "Logo",
             "gridPos": {"x": 0, "y": 15, "w": 4, "h": 2},
             "options": {"content": "<div style=\"background-image:url(logo.png)\"></div>"}},
            {"id": 32, "type": "dashlist", "title": "Links",
             "gridPos": {"x": 4, "y": 15, "w": 8, "h": 2}},
            {"id": 33, "type": "stat", "title": "Banner",
             "gridPos": {"x": 12, "y": 15, "w": 12, "h": 2}, "datasource": ds,
             "targets": [{"refId": "A", "datasource": ds,
                "rawSql": "SELECT value FROM metric_table WHERE $__timeFilter(time_column)"}]}
        ]
    })
}

/// The headline fidelity test (viz grafana-dashboard-fidelity scope, testing plan): a real Grafana IAQ
/// export → commit on a real node → the stored `Dashboard` is wired, macro-free, grid-aligned, placeholder-
/// free, and honestly reported. No mocks.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn iaq_import_is_wired_macro_free_grid_aligned_and_honest() {
    let (gw, key) = gateway().await;
    seed_datasource(&gw.node, "acme", "pdnsw").await;
    let tok = token(&key, "user:ada", "acme", IMPORT_CAPS);

    let body = json!({
        "json": grafana_iaq_export(),
        "id": "iaq",
        "mappings": [{ "type": "postgres", "uid": "pdnsw-uid", "mappedTo": "pdnsw" }]
    });
    let resp = router(gw.clone())
        .oneshot(bearer(json_post("/dashboards/import", body), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let out: Value = json_body(resp).await;

    let cells = out["dashboard"]["cells"].as_array().unwrap();
    // 9 panels − 3 dropped placeholders = 6 cells (1 row, 3 timeseries, 1 stat, 1 text).
    assert_eq!(
        cells.len(),
        6,
        "dropped placeholders must be absent from cells"
    );
    let view_count = |v: &str| cells.iter().filter(|c| c["view"] == v).count();
    assert_eq!(view_count("timeseries"), 3);
    assert_eq!(view_count("stat"), 1);
    assert_eq!(view_count("row"), 1);
    assert_eq!(view_count("text"), 1);
    assert_eq!(
        view_count("json"),
        0,
        "no unsupported/'no template' placeholder cells"
    );

    // WIRED-AND-DRAWING: every data cell's target is executable — 0 `tool:""`, and macro-free.
    for c in cells
        .iter()
        .filter(|c| !c["sources"].as_array().unwrap().is_empty())
    {
        for t in c["sources"].as_array().unwrap() {
            assert_eq!(t["tool"], "federation.query", "cell {} unwired", c["i"]);
            let sql = t["args"]["sql"].as_str().unwrap();
            assert!(
                !sql.contains("$__time("),
                "untranslated $__time in {}",
                c["i"]
            );
            assert!(
                !sql.contains("$__timeFilter("),
                "untranslated $__timeFilter in {}",
                c["i"]
            );
        }
    }
    // The bounded host window replaced $__timeFilter on a chart (the 30 s-cancel fix).
    let ts = cells.iter().find(|c| c["view"] == "timeseries").unwrap();
    let ts_sql = ts["sources"][0]["args"]["sql"].as_str().unwrap();
    assert!(ts_sql.contains("to_timestamp($__from / 1000.0)"));

    // GRID: nothing exceeds the 12-col width, and the three charts pack 3-across (distinct columns).
    for c in cells {
        let (x, w) = (c["x"].as_u64().unwrap(), c["w"].as_u64().unwrap());
        assert!(
            x + w <= 12,
            "cell {} overflows the 12-col grid (x={x} w={w})",
            c["i"]
        );
    }
    let mut chart_xs: Vec<u64> = cells
        .iter()
        .filter(|c| c["view"] == "timeseries")
        .map(|c| c["x"].as_u64().unwrap())
        .collect();
    chart_xs.sort_unstable();
    chart_xs.dedup();
    assert_eq!(
        chart_xs.len(),
        3,
        "the 3 charts must occupy 3 distinct columns"
    );

    // The `text` note carries its content to the sanitized text view.
    let text = cells.iter().find(|c| c["view"] == "text").unwrap();
    assert!(text["options"]["content"]
        .as_str()
        .unwrap()
        .contains("Level 2"));

    // HONEST REPORT: every drop/degrade has a line.
    let degraded = out["report"]["degraded"].as_array().unwrap();
    let has = |needle: &str| {
        degraded
            .iter()
            .any(|d| d["detail"].as_str().unwrap().contains(needle))
    };
    assert!(has("dashlist"), "dropped dashlist unreported");
    assert!(has("text panel"), "dropped logo text unreported");
    assert!(has("metric_table"), "dropped banner unreported");
    assert!(has("annotation plane"), "annotation drop unreported");
    assert!(has("auto-refresh"), "refresh drop unreported");
    assert!(has("graphTooltip"), "graphTooltip drop unreported");

    // TOOLBAR: the wired window implies the date picker; the source refreshed → show the control.
    assert_eq!(out["dashboard"]["toolbar"]["dateSelect"], true);
    assert_eq!(out["dashboard"]["toolbar"]["refreshRate"], true);
    // Timezone preserved (not degraded).
    assert_eq!(out["dashboard"]["timezone"], "Australia/Sydney");
}

async fn seed_datasource(node: &Node, ws: &str, name: &str) {
    let ds = Datasource::new(name, "postgres", "127.0.0.1:5432", "secret/x", NOW);
    lb_host::put_datasource(&node.store, ws, &ds)
        .await
        .expect("seed datasource");
}

/// The headline: preview reports the datasource + degraded heatmap; commit maps + writes; export
/// round-trips it back to Grafana JSON semantically stable (migration applied, passthrough survived).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn import_preview_commit_export_round_trip() {
    let (gw, key) = gateway().await;
    seed_datasource(&gw.node, "acme", "our-metrics").await;
    let tok = token(&key, "user:ada", "acme", IMPORT_CAPS);

    // --- PREVIEW (no mappings) ---
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/dashboards/import", json!({ "json": grafana_export() })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let preview: Value = json_body(resp).await;
    // no write on preview
    assert_eq!(preview["id"], "");
    // the migration normalized from v30
    assert_eq!(preview["report"]["migratedFrom"], 30);
    // one datasource to remap (DS_PROM resolved to its own token as a uid)
    let ds = preview["report"]["datasources"].as_array().unwrap();
    assert_eq!(ds.len(), 1);
    let ds_uid = ds[0]["uid"].as_str().unwrap().to_string();
    // heatmap degraded honestly
    let degraded = preview["report"]["degraded"].as_array().unwrap();
    assert!(degraded
        .iter()
        .any(|d| d["detail"].as_str().unwrap().contains("heatmap")));
    // 1 mapped panel (graph→timeseries); heatmap is the json placeholder, not counted
    assert_eq!(preview["report"]["mappedPanels"], 1);

    // --- COMMIT (bind DS_PROM → our-metrics) ---
    let commit_body = json!({
        "json": grafana_export(),
        "id": "ops-imported",
        "mappings": [{ "type": "prometheus", "uid": ds_uid, "mappedTo": "our-metrics" }]
    });
    let resp = router(gw.clone())
        .oneshot(bearer(json_post("/dashboards/import", commit_body), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let commit: Value = json_body(resp).await;
    assert_eq!(commit["id"], "ops-imported");
    let cells = commit["dashboard"]["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 2);
    // graph became timeseries; its datasource remapped to our source
    let ts = cells.iter().find(|c| c["view"] == "timeseries").unwrap();
    assert_eq!(
        ts["sources"][0]["datasource"],
        json!({"uid": "our-metrics"})
    );
    // heatmap is the honest json placeholder recording the original type
    let ph = cells.iter().find(|c| c["view"] == "json").unwrap();
    assert_eq!(ph["options"]["unsupportedType"], "heatmap");

    // --- EXPORT (round-trip) ---
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/dashboards/ops-imported/export"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let exported: Value = json_body(resp).await;
    assert_eq!(exported["schemaVersion"], grafana_map_pinned());
    let panels = exported["panels"].as_array().unwrap();
    // the timeseries panel exports with its migrated type + the preserved unknown field (passthrough)
    let ts_panel = panels.iter().find(|p| p["type"] == "timeseries").unwrap();
    assert_eq!(ts_panel["customPluginField"], json!({"keep": true}));
    assert_eq!(
        ts_panel["fieldConfig"],
        json!({"defaults": {"unit": "percent"}})
    );
    // the heatmap exports back to its ORIGINAL type, not "json" (honest round-trip)
    assert!(panels.iter().any(|p| p["type"] == "heatmap"));
}

/// Workspace isolation (the hard wall): a ws-B import can never bind a ws-A datasource. `our-metrics`
/// exists only in `acme`; a `beta` caller mapping to it is refused (`403`), the import never writes.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_import_cannot_bind_ws_a_datasource() {
    let (gw, key) = gateway().await;
    seed_datasource(&gw.node, "acme", "our-metrics").await; // acme-only
    let tok_beta = token(&key, "user:bob", "beta", IMPORT_CAPS);

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/dashboards/import",
                json!({
                    "json": grafana_export(),
                    "id": "sneaky",
                    "mappings": [{ "type": "prometheus", "uid": "whatever", "mappedTo": "our-metrics" }]
                }),
            ),
            &tok_beta,
        ))
        .await
        .unwrap();
    // Refused — `our-metrics` is invisible in `beta` (the hard wall).
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // …and nothing was written in beta.
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/dashboards/sneaky"), &tok_beta))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// Capability deny (mandatory): import without `mcp:dashboard.import:call` is denied opaquely; export
/// without `mcp:dashboard.export:call` is denied.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn import_and_export_require_their_caps() {
    let (gw, key) = gateway().await;
    // A token holding SAVE but NOT import — import must still be denied.
    let no_import = token(&key, "user:ada", "acme", &["mcp:dashboard.save:call"]);
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/dashboards/import",
                json!({ "json": grafana_export(), "id": "x" }),
            ),
            &no_import,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Export without its cap is denied (even holding get).
    let no_export = token(&key, "user:ada", "acme", &["mcp:dashboard.get:call"]);
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/dashboards/any/export"), &no_export))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

/// A v2 (app-platform) export is rejected with a pointer (400), never mis-imported as v1.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn v2_app_platform_export_rejected() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", IMPORT_CAPS);
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/dashboards/import",
                json!({ "json": {"apiVersion": "dashboard.grafana.app/v2beta1", "spec": {}} }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let msg = body_text(resp).await;
    assert!(msg.contains("classic") || msg.contains("v2"));
}

/// The `grafana_map` pinned schemaVersion the export advertises (kept in sync with the crate).
fn grafana_map_pinned() -> u64 {
    grafana_map::PINNED_SCHEMA_VERSION
}

// Ensure the shared `Node`/`Qos` symbols stay referenced (mirrors dashboard_routes_test imports so a
// future SSE-seeding case can reuse them without a churny import edit).
#[allow(dead_code)]
fn _sig(_n: Arc<Node>, _q: Qos, _r: NodeRole) {}
