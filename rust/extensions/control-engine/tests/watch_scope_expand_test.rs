//! Regression — `control-engine.watch` with an EMPTY scope must expand to the appliance's
//! full component-UID set BEFORE arming, or the ce-studio engine (which only pushes COV
//! frames for EXPLICITLY subscribed components) streams zero value frames and the canvas
//! shows no live values. Root cause + live proof:
//! `docs/debugging/frontend/ce-canvas-empty-cov-scope-no-live-values.md`.
//!
//! Rule 9 (no mocks/fakes of node behavior): this drives the REAL `expand_scope` →
//! `tools::raw_tree` → `scope_uids::collect` path over a REAL HTTP round-trip. The server is
//! a real `axum` app serving a **captured-real** `/api/v0/nodes` envelope (the exact bytes
//! `GET http://127.0.0.1:7979/api/v0/nodes?depth=-1&withEdges=true` returned on the live
//! engine — a synthetic root `uid 0` over a nested `children` tree). Nothing about the
//! control-engine sidecar or the LB node is stubbed; only a genuine external HTTP endpoint
//! (the engine) is stood up locally, which is exactly what rule 9 permits for an external.

use std::net::SocketAddr;

use axum::{routing::get, Json, Router};
use control_engine::watch::series::target;
use control_engine::watch::verb::expand_scope;
use serde_json::{json, Value};

/// The captured-real `/nodes` envelope (see the module doc). Root `uid 0` + a nested
/// `Services` subtree + the six top-level components the handover proved stream frames.
fn captured_nodes_envelope() -> Value {
    json!({ "data": { "edges": [], "nodes": [{
        "uid": 0, "name": "root", "children": [
            { "uid": 100000, "name": "Services", "children": [
                { "uid": 100001, "name": "ZenohService", "children": null },
                { "uid": 100002, "name": "bacnetService", "children": null },
                { "uid": 100003, "name": "jsScriptStore", "children": null },
                { "uid": 100004, "name": "McpService", "children": null },
                { "uid": 100005, "name": "alarm", "children": null },
                { "uid": 100006, "name": "RestService", "children": null },
                { "uid": 100007, "name": "mqttService", "children": null }
            ]},
            { "uid": 100008, "name": "changeOfState", "children": null },
            { "uid": 100009, "name": "limitAlarm", "children": null },
            { "uid": 100010, "name": "random", "children": null },
            { "uid": 100011, "name": "dewpoint", "children": null },
            { "uid": 100012, "name": "cron", "children": null },
            { "uid": 100013, "name": "bacnetDevice", "children": null }
        ]
    }]}})
}

/// Boot a real HTTP server on an ephemeral port that answers the `/api/v0/nodes` route the
/// tolerant tree fetch hits (both the root `?depth=..` and the uid-keyed form), returning the
/// captured envelope. Returns the `host:port` base `expand_scope` connects to.
async fn serve_engine() -> String {
    let app = Router::new()
        .route(
            "/api/v0/nodes",
            get(|| async { Json(captured_nodes_envelope()) }),
        )
        .route(
            "/api/v0/nodes/uid/{uid}",
            get(|| async { Json(captured_nodes_envelope()) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind engine stub");
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("{}:{}", addr.ip(), addr.port())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_scope_expands_to_every_component_uid() {
    let base = serve_engine().await;

    // The UI's default watch: `{ appliance }`, no scope → "watch the whole appliance".
    let input = json!({ "appliance": "aaaa" });
    let expanded = expand_scope(&base, &input).await;

    let components = expanded["scope"]["components"]
        .as_array()
        .expect("expand_scope injects scope.components for an empty scope");

    // The six frame-bearing children the handover verified are all subscribed now...
    for want in [100008u64, 100009, 100010, 100011, 100012, 100013] {
        assert!(
            components.iter().any(|c| c.as_u64() == Some(want)),
            "expanded scope missing {want}: {components:?}"
        );
    }
    // ...the nested Services subtree is included...
    assert!(components.iter().any(|c| c.as_u64() == Some(100000)));
    assert!(components.iter().any(|c| c.as_u64() == Some(100001)));
    // ...and the synthetic root (uid 0, no COV) is NOT subscribed.
    assert!(
        components.iter().all(|c| c.as_u64() != Some(0)),
        "root uid 0 must not be subscribed: {components:?}"
    );

    // The expanded scope MUST resolve to a different series than the empty one — this is the
    // exact regression: an empty-scope subscribe (the old series) carried zero frames; the
    // populated scope (the new series) is what the pump enumerates and streams.
    let empty_series = target("aaaa", &json!({ "appliance": "aaaa" })).series;
    let expanded_series = target("aaaa", &expanded).series;
    assert_ne!(
        empty_series, expanded_series,
        "expanded scope must key a different (populated) series than the empty scope"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_scope_is_never_widened() {
    let base = serve_engine().await;
    // A caller that names specific components is deliberately narrowing — we must honour it
    // verbatim and NOT replace it with the whole tree.
    let input = json!({ "appliance": "aaaa", "scope": { "components": [100010] } });
    let expanded = expand_scope(&base, &input).await;
    assert_eq!(
        expanded["scope"]["components"],
        json!([100010]),
        "an explicit component scope must pass through unchanged"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unreachable_engine_falls_back_to_the_given_scope() {
    // A dead engine base: the tree fetch fails, but the watch must still arm (a gap, not a
    // failed call). `expand_scope` returns the input unchanged rather than erroring.
    let input = json!({ "appliance": "aaaa" });
    let out = expand_scope("127.0.0.1:1", &input).await;
    assert_eq!(
        out, input,
        "a tree-fetch failure is non-fatal: input passes through"
    );
}
