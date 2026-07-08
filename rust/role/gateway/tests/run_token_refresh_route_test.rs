//! `POST /agent/runs/{id}/token/refresh` over the REAL gateway (agent-key-lifecycle D2/D3). No
//! mock: a real `Node`, a real signed run-scoped token, a real run job seeded into the store.
//! Proves:
//!   - **happy path:** a run-scoped token for a LIVE run refreshes → `200` with `{token,
//!     refresh_at_sec}`; the fresh token verifies and carries the same `run_id`.
//!   - **run-status gate (D3, mandatory):** the same token is REFUSED (401, opaque) once the run
//!     is `Cancelled` — hard cancel is instant, not TTL-bound. This is the load-bearing D3 test.
//!   - **path-vs-claim mismatch:** a token whose `run_id` ≠ the path id → `400`.
//!   - **non-run token:** an ordinary session token (no `run_id`) hitting the route → `400`.
//!   - **workspace isolation:** a ws-B run token cannot refresh against a ws-A run path (the
//!     token's ws wins; the path/claim mismatch surfaces as a 400, and the run is never touched).

mod common;

use axum::http::StatusCode;
use common::*;
use lb_jobs::{cancel, create, load, Job, JobStatus};
use lb_role_gateway::router;
use tower::ServiceExt;

/// Seed a `Running` agent-session run job so the refresh + verify paths have something to consult.
async fn seed_run(gw: &lb_role_gateway::Gateway, ws: &str, id: &str) {
    create(
        &gw.node.store,
        ws,
        &Job::new(id, "agent-session", "goal", 1),
    )
    .await
    .expect("seed run job");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_live_run_token_refreshes_to_a_same_shape_token() {
    let (gw, key) = gateway().await;
    let ws = "gw-refresh";
    seed_run(&gw, ws, "run-1").await;
    let tok = run_token(
        &key,
        "agent:session",
        ws,
        &["mcp:tools.catalog:call"],
        None,
        "run-1",
    );

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/agent/runs/run-1/token/refresh", serde_json::json!({})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "refresh a live run → 200");
    let body: serde_json::Value = json_body(resp).await;
    let fresh = body["token"].as_str().expect("fresh token in reply");
    assert!(fresh != tok, "the refreshed token is a NEW bearer");
    assert!(
        body["refresh_at_sec"].as_u64().is_some(),
        "the reply carries the next refresh timestamp"
    );
    // The fresh token verifies against the same key + carries the same run_id + ws.
    let p = lb_auth::verify(&key, fresh, NOW).expect("fresh token verifies");
    assert_eq!(p.run_id(), Some("run-1"));
    assert_eq!(p.ws(), ws);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_terminal_run_token_is_refused_at_refresh() {
    // D3 (mandatory): a token whose run is CANCELLED is refused even if unexpired. Hard cancel is
    // instant — the TTL is the belt, run-status is the braces.
    let (gw, key) = gateway().await;
    let ws = "gw-refresh-cancel";
    seed_run(&gw, ws, "run-c").await;
    cancel(&gw.node.store, ws, "run-c").await.expect("cancel");
    assert_eq!(
        load(&gw.node.store, ws, "run-c")
            .await
            .unwrap()
            .unwrap()
            .status,
        JobStatus::Cancelled
    );
    let tok = run_token(
        &key,
        "agent:session",
        ws,
        &["mcp:tools.catalog:call"],
        None,
        "run-c",
    );

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/agent/runs/run-c/token/refresh", serde_json::json!({})),
            &tok,
        ))
        .await
        .unwrap();
    // The run-status gate fires inside `authenticate` → opaque 401 (no oracle: indistinguishable
    // from an expired/revoked credential).
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "a cancelled run's refresh is refused (D3)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_run_token_whose_path_id_differs_from_its_claim_is_400() {
    let (gw, key) = gateway().await;
    let ws = "gw-refresh-mismatch";
    seed_run(&gw, ws, "run-a").await;
    seed_run(&gw, ws, "run-b").await;
    // The token is scoped to run-a but the PATH names run-b. The mismatch is a client bug → 400
    // (the bearer authenticated, so this is not an attack surface; it's a wired-wrong shim).
    let tok = run_token(
        &key,
        "agent:session",
        ws,
        &["mcp:tools.catalog:call"],
        None,
        "run-a",
    );
    let resp = router(gw)
        .oneshot(bearer(
            json_post("/agent/runs/run-b/token/refresh", serde_json::json!({})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "path ≠ claim → 400");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_ordinary_session_token_on_the_refresh_route_is_400() {
    let (gw, key) = gateway().await;
    let ws = "gw-refresh-norun";
    seed_run(&gw, ws, "run-n").await;
    // An ordinary session token (no run_id claim) hitting the route → 400. The route is for
    // run-scoped tokens only; a browser session has no business here.
    let tok = token(&key, "user:ada", ws, &["mcp:tools.catalog:call"]);
    let resp = router(gw)
        .oneshot(bearer(
            json_post("/agent/runs/run-n/token/refresh", serde_json::json!({})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "non-run token on the refresh route → 400"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_terminal_run_token_is_refused_at_mcp_call_too() {
    // The run-status gate fires on EVERY verify chokepoint — not just refresh. A bridged
    // `tools/call` (the path the shim drives) over a terminal-run token is refused identically.
    // This proves the wall is at `verify_token`, not at the route.
    let (gw, key) = gateway().await;
    let ws = "gw-refresh-mcp";
    seed_run(&gw, ws, "run-m").await;
    cancel(&gw.node.store, ws, "run-m").await.expect("cancel");
    let tok = run_token(
        &key,
        "agent:session",
        ws,
        &["mcp:tools.catalog:call"],
        None,
        "run-m",
    );

    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/mcp/call",
                serde_json::json!({"tool":"tools.catalog","args":{}}),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "a cancelled run's token is refused at /mcp/call too (D3 at the verify chokepoint)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_ws_b_run_token_cannot_refresh_a_ws_a_run() {
    // Workspace isolation (mandatory): the token's ws is the hard wall — a ws-B run token cannot
    // reach a ws-A run even if it names the same run_id. The verify + the path/claim mismatch
    // compose: the token's ws-B claim + the run-A's path produce a 400 (the path run lives in
    // ws-A, not the token's ws-B), and ws-A's run is never consulted.
    let (gw, key) = gateway().await;
    seed_run(&gw, "ws-a", "run-x").await;
    let tok_b = run_token(
        &key,
        "agent:session",
        "ws-b",
        &["mcp:tools.catalog:call"],
        None,
        "run-x",
    );
    let resp = router(gw)
        .oneshot(bearer(
            json_post("/agent/runs/run-x/token/refresh", serde_json::json!({})),
            &tok_b,
        ))
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "a ws-B token on a ws-A run does not succeed: {}",
        resp.status()
    );
}
