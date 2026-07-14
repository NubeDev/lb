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
