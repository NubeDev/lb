//! The chain routes over the real gateway (rules-workbench scope, Phase 2) — the `chains.*` CRUD +
//! run + the per-step run snapshot, end to end. Mirrors the host chains tests at the transport
//! boundary: the CRUD round-trip, capability-deny per verb, two-session workspace isolation, a cyclic
//! DAG rejected at save (`400` with the validation message — the canvas inline error), and the
//! run → runs.get snapshot round-trip (the settle-colouring source). The gateway re-checks every gate
//! server-side; the workspace + principal come from the token (§7).
//!
//! A chain's steps run saved rules, so each chain run needs a saved rule first — seeded in-test via
//! `lb_host::call_tool(..., "rules.save", ...)` (we may call the host directly in a Rust test).

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

/// The six chains caps + the `rules.run` + store caps a chain run needs (a step runs a saved rule
/// under `caller ∩ grant`). The dev member holds these in real life.
const CAPS: &[&str] = &[
    "mcp:chains.save:call",
    "mcp:chains.run:call",
    "mcp:chains.get:call",
    "mcp:chains.list:call",
    "mcp:chains.delete:call",
    "mcp:chains.runs.get:call",
    "mcp:rules.save:call",
    "mcp:rules.run:call",
    "store:chain:write",
    "store:chain:read",
    "store:rule:write",
    "store:rule:read",
];

/// Seed a saved rule a chain step references, through the real `rules.save` host verb.
async fn seed_rule(node: &Arc<Node>, key: &SigningKey, ws: &str, name: &str, body: &str) {
    let p = lb_auth::verify(key, &token(key, "user:ada", ws, CAPS), common::NOW).unwrap();
    lb_host::call_tool(
        node,
        &p,
        ws,
        "rules.save",
        &json!({ "id": name, "name": name, "body": body }).to_string(),
    )
    .await
    .expect("seed rule");
}

/// A chain body (the host sets the workspace from the token; we omit it).
fn chain_body(id: &str, steps: Value) -> Value {
    json!({ "id": id, "name": id, "steps": steps })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn chains_crud_round_trip_over_the_gateway() {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ws = "acme";
    let tok = token(&key, "user:ada", ws, CAPS);
    seed_rule(&node, &key, ws, "r", r#"emit(#{ level: "info" });"#).await;

    // create
    let body = chain_body("pipe", json!([{ "id": "a", "rule": "r", "needs": [] }]));
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(json_post("/chains", body), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let saved: Value = json_body(resp).await;
    assert_eq!(saved["id"], "pipe");

    // get loads it
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/chains/pipe"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let c: Value = json_body(resp).await;
    assert_eq!(c["id"], "pipe");
    assert_eq!(c["steps"][0]["id"], "a");

    // list route responds 200 with the `{chains:[...]}` roster, which CONTAINS the saved chain — the
    // real round-trip (the host `chains_list` envelope-unwrap was fixed this slice; see the session
    // doc). Stays workspace-walled (the isolation test asserts ws-B's list is empty).
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/chains"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    let ids: Vec<&str> = body["chains"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"pipe"), "the roster lists the saved chain");

    // delete → 204, then get is 404
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(delete_req("/chains/pipe"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/chains/pipe"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_chains_verb_is_denied_without_its_cap() {
    let (gw, key) = gateway().await;
    let ws = "acme";
    // The route, the request, and the cap it requires.
    let cases: Vec<(&str, axum::http::Request<axum::body::Body>)> = vec![
        ("mcp:chains.list:call", get_req("/chains")),
        ("mcp:chains.get:call", get_req("/chains/c")),
        (
            "mcp:chains.save:call",
            json_post("/chains", chain_body("c", json!([]))),
        ),
        ("mcp:chains.delete:call", delete_req("/chains/c")),
        ("mcp:chains.run:call", json_post("/chains/c/run", json!({}))),
        ("mcp:chains.runs.get:call", get_req("/chains/c/runs/r")),
    ];
    for (cap, req) in cases {
        // A token holding EVERY cap except the one this route requires.
        let caps: Vec<&str> = CAPS.iter().copied().filter(|c| *c != cap).collect();
        let tok = token(&key, "user:ada", ws, &caps);
        let resp = router(gw.clone()).oneshot(bearer(req, &tok)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "missing {cap} → 403");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn two_sessions_are_workspace_isolated() {
    // One node, two sessions in different workspaces — ws-B sees none of ws-A's chains.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    seed_rule(&node, &key, "ws-a", "r", r#"emit(#{ level: "info" });"#).await;
    let ada = token(&key, "user:ada", "ws-a", CAPS);
    let ben = token(&key, "user:ben", "ws-b", CAPS);

    router(gateway_on(node.clone(), &key))
        .oneshot(bearer(
            json_post(
                "/chains",
                chain_body("priv", json!([{ "id": "a", "rule": "r", "needs": [] }])),
            ),
            &ada,
        ))
        .await
        .unwrap();

    // Ben (ws-B) gets a 404 for ws-A's chain and an empty roster.
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/chains/priv"), &ben))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/chains"), &ben))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    assert!(body["chains"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_cyclic_dag_is_400_with_the_validation_message() {
    // The canvas inline-error source: an invalid DAG → `BadInput` → `400` with the message verbatim.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ws = "acme";
    let tok = token(&key, "user:ada", ws, CAPS);
    seed_rule(&node, &key, ws, "r", r#"emit(#{ level: "info" });"#).await;

    let cyclic = chain_body(
        "bad",
        json!([
            { "id": "a", "rule": "r", "needs": ["b"] },
            { "id": "b", "rule": "r", "needs": ["a"] },
        ]),
    );
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(json_post("/chains", cyclic), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "a cycle → 400");
    let msg: String = String::from_utf8(
        axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        msg.to_lowercase().contains("cycle"),
        "the validation message is shown verbatim (got: {msg})"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn run_then_runs_get_snapshot_round_trips() {
    // The settle-colouring source: run a chain, then read the per-step snapshot to colour the canvas.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ws = "acme";
    let tok = token(&key, "user:ada", ws, CAPS);
    seed_rule(&node, &key, ws, "r", r#"emit(#{ level: "info" });"#).await;

    // A two-step chain a → b.
    router(gateway_on(node.clone(), &key))
        .oneshot(bearer(
            json_post(
                "/chains",
                chain_body(
                    "run-me",
                    json!([
                        { "id": "a", "rule": "r", "needs": [] },
                        { "id": "b", "rule": "r", "needs": ["a"] },
                    ]),
                ),
            ),
            &tok,
        ))
        .await
        .unwrap();

    // Run → {run_id}.
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(json_post("/chains/run-me/run", json!({})), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let run: Value = json_body(resp).await;
    let run_id = run["run_id"].as_str().expect("a run id").to_string();

    // The snapshot the canvas polls: a terminal status + per-step outcomes.
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(
            get_req(&format!("/chains/run-me/runs/{run_id}")),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let snap: Value = json_body(resp).await;
    assert_eq!(snap["status"], "success");
    let steps = snap["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2);
    for s in steps {
        assert_eq!(s["outcome"], "ok", "every step settles ok (→ green)");
    }
}
