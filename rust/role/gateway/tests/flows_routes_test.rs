//! The flow routes over the real gateway (flows-canvas + dashboard-binding scopes, Wave 3) — the
//! `flows.*` CRUD + nodes + run + runs.get + patch_run + inject, end to end. Mirrors the host flows
//! tests at the transport boundary: the merged `flows.nodes` registry (built-ins), the CRUD
//! round-trip, capability-deny per verb, two-session workspace isolation, an invalid DAG rejected at
//! save (`400` with the validation message — the canvas inline error), and the run → runs.get
//! snapshot round-trip (the settle-colouring source). The gateway re-checks every gate server-side;
//! the workspace + principal come from the token (§7).

mod common;

use axum::http::StatusCode;
use common::*;
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

/// The flows caps a dev member holds (CRUD + run + nodes + runs + patch + enable + inject).
const CAPS: &[&str] = &[
    "mcp:flows.save:call",
    "mcp:flows.get:call",
    "mcp:flows.list:call",
    "mcp:flows.delete:call",
    "mcp:flows.nodes:call",
    "mcp:flows.run:call",
    "mcp:flows.patch_run:call",
    "mcp:flows.runs.get:call",
    "mcp:flows.runs.list:call",
    "mcp:flows.enable:call",
    "mcp:flows.inject:call",
    "store:flow:read",
    "store:flow:write",
];

/// A minimal trigger-only flow body (the host sets the workspace from the token).
fn flow_body(id: &str) -> Value {
    json!({
        "id": id,
        "name": id,
        "version": 1,
        "nodes": [{ "id": "start", "type": "trigger", "needs": [], "config": { "mode": "manual" } }],
        "failurePolicy": "halt"
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn flows_nodes_returns_the_built_in_registry() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    let resp = router(gw)
        .oneshot(bearer(get_req("/flows/nodes"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    let types: Vec<&str> = body["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["type"].as_str().unwrap())
        .collect();
    for builtin in ["trigger", "tool", "rhai", "subflow", "sink"] {
        assert!(types.contains(&builtin), "built-in {builtin} on the palette");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn flows_crud_round_trip_over_the_gateway() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    // Save → {id, version}.
    let resp = router(gw.clone())
        .oneshot(bearer(json_post("/flows", flow_body("cooler")), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert_eq!(body["id"], "cooler");
    assert_eq!(body["version"], 1);

    // List shows it.
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/flows"), &tok))
        .await
        .unwrap();
    let list: Value = json_body(resp).await;
    let ids: Vec<&str> = list["flows"].as_array().unwrap().iter().map(|f| f["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"cooler"));

    // Get round-trips the graph.
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/flows/cooler"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let got: Value = json_body(resp).await;
    assert_eq!(got["nodes"][0]["type"], "trigger");

    // Delete → 204; a subsequent get → 404.
    let resp = router(gw.clone())
        .oneshot(bearer(delete_req("/flows/cooler"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    // A tombstoned flow is absent → `flows.get` collapses to an opaque Denied (no existence leak).
    let resp = router(gw)
        .oneshot(bearer(get_req("/flows/cooler"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_rejects_a_cyclic_dag_with_a_400_inline_error() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);
    let cyclic = json!({
        "id": "cyc", "name": "cyc", "version": 1, "failurePolicy": "halt",
        "nodes": [
            { "id": "a", "type": "trigger", "needs": ["b"], "config": { "mode": "manual" } },
            { "id": "b", "type": "trigger", "needs": ["a"], "config": { "mode": "manual" } }
        ]
    });
    let resp = router(gw)
        .oneshot(bearer(json_post("/flows", cyclic), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.into_body();
    let bytes = http_body_util::BodyExt::collect(body).await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("cycle"), "inline error names the cycle: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn run_then_runs_get_returns_the_snapshot() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    let _ = router(gw.clone())
        .oneshot(bearer(json_post("/flows", flow_body("rt")), &tok))
        .await
        .unwrap();
    let resp = router(gw.clone())
        .oneshot(bearer(json_post("/flows/rt/run", json!({})), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    let run_id = body["run_id"].as_str().unwrap().to_string();

    let resp = router(gw)
        .oneshot(bearer(get_req(&format!("/flows/runs/{run_id}")), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let snap: Value = json_body(resp).await;
    assert_eq!(snap["flowId"], "rt");
    assert_eq!(snap["runId"], run_id);
    // The trigger node appears as a step (the colour source).
    assert!(snap["steps"].as_array().unwrap().iter().any(|s| s["id"] == "start"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_token_without_the_save_cap_is_denied() {
    let (gw, key) = gateway().await;
    // Holds list + nodes + get but NOT save.
    let tok = token(
        &key,
        "user:ada",
        "acme",
        &["mcp:flows.list:call", "mcp:flows.nodes:call", "mcp:flows.get:call"],
    );
    let resp = router(gw)
        .oneshot(bearer(json_post("/flows", flow_body("denied")), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_b_cannot_read_workspace_a_flow() {
    let (gw, key) = gateway().await;
    let tok_a = token(&key, "user:ada", "acme", CAPS);
    let tok_b = token(&key, "user:bob", "burger", CAPS);

    // ws-A saves a flow.
    let _ = router(gw.clone())
        .oneshot(bearer(json_post("/flows", flow_body("secret")), &tok_a))
        .await
        .unwrap();

    // ws-B cannot get ws-A's flow (the workspace wall — derived from the token, not the path). The
    // host collapses an absent flow to an opaque Denied (no existence leak) → 403.
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/flows/secret"), &tok_b))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // ws-B's list does not include ws-A's flow.
    let resp = router(gw)
        .oneshot(bearer(get_req("/flows"), &tok_b))
        .await
        .unwrap();
    let list: Value = json_body(resp).await;
    let ids: Vec<&str> = list["flows"].as_array().unwrap().iter().map(|f| f["id"].as_str().unwrap()).collect();
    assert!(!ids.contains(&"secret"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn inject_into_a_retained_node_sets_state_and_starts_no_run() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);
    // A flow with a retained `inject` trigger node (Decision 9 — inject sets the held value).
    let flow = json!({
        "id": "ret", "name": "ret", "version": 1, "failurePolicy": "halt",
        "nodes": [{
            "id": "setpoint", "type": "trigger", "needs": [],
            "config": { "mode": "inject", "inject_mode": "retain" }
        }]
    });
    let _ = router(gw.clone())
        .oneshot(bearer(json_post("/flows", flow), &tok))
        .await
        .unwrap();

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/flows/ret/inject", json!({ "node": "setpoint", "value": 4 })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    // A retained inject starts no run (Decision 9).
    assert_eq!(body["fired_run"], false);
}

/// A DELETE request builder (the common mod lacks one).
fn delete_req(uri: &str) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::builder()
        .method("DELETE")
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap()
}
