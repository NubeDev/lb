//! `POST /runs/{job}/{op}` over the REAL gateway (agent-dock run controls) — stop / pause / resume a
//! run. No mock: a real `Node`, a real signed token, a real run job seeded into the store. Proves:
//!   - **happy path:** a principal WITH `mcp:agent.control:call` pauses a Running run → `204`, and the
//!     durable status is `Suspended`; resume → `204` → `Running`.
//!   - **capability deny:** WITHOUT the cap → opaque `403` (the host self-gate; the route never leaks).
//!   - **unknown op:** `POST /runs/{job}/frobnicate` → `400`.
//!   - **workspace isolation:** a ws-B token controlling a ws-A run acts in ws-B (the token's ws) —
//!     ws-A's run is never touched (the hard wall §7).

mod common;

use axum::http::StatusCode;
use common::*;
use lb_jobs::{create, load, Job, JobStatus};
use lb_role_gateway::router;
use tower::ServiceExt;

const CONTROL: &[&str] = &["mcp:agent.control:call"];

/// Seed a `Running` agent-session run job into `ws` so a control route has something to act on.
async fn seed_run(gw: &lb_role_gateway::Gateway, ws: &str, id: &str) {
    create(
        &gw.node.store,
        ws,
        &Job::new(id, "agent-session", "goal", 1),
    )
    .await
    .expect("seed run job");
}

async fn status(gw: &lb_role_gateway::Gateway, ws: &str, id: &str) -> JobStatus {
    load(&gw.node.store, ws, id).await.unwrap().unwrap().status
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pause_then_resume_over_the_route() {
    let (gw, key) = gateway().await;
    let ws = "gw-ctl";
    seed_run(&gw, ws, "run-1").await;
    let tok = token(&key, "user:ada", ws, CONTROL);

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/runs/run-1/pause", serde_json::json!({})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "pause returns 204");
    assert_eq!(status(&gw, ws, "run-1").await, JobStatus::Suspended);

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/runs/run-1/resume", serde_json::json!({})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "resume returns 204");
    assert_eq!(status(&gw, ws, "run-1").await, JobStatus::Running);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_over_the_route_cancels() {
    let (gw, key) = gateway().await;
    let ws = "gw-ctl-stop";
    seed_run(&gw, ws, "run-s").await;
    let tok = token(&key, "user:ada", ws, CONTROL);

    // `cancel` and `stop` are aliases; use the friendly `cancel` route segment.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/runs/run-s/cancel", serde_json::json!({})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(status(&gw, ws, "run-s").await, JobStatus::Cancelled);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn control_without_the_cap_is_opaque_403() {
    let (gw, key) = gateway().await;
    let ws = "gw-ctl-deny";
    seed_run(&gw, ws, "run-d").await;
    // A token WITHOUT mcp:agent.control:call.
    let tok = token(&key, "user:bob", ws, &["mcp:series.latest:call"]);

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/runs/run-d/pause", serde_json::json!({})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "no control cap → 403");
    assert_eq!(body_text(resp).await, "denied", "the 403 body is opaque");
    // The run is untouched — a denied control never moved the status.
    assert_eq!(status(&gw, ws, "run-d").await, JobStatus::Running);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unknown_op_is_400() {
    let (gw, key) = gateway().await;
    let ws = "gw-ctl-badop";
    seed_run(&gw, ws, "run-b").await;
    let tok = token(&key, "user:ada", ws, CONTROL);

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/runs/run-b/frobnicate", serde_json::json!({})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "an unknown op is 400"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_ws_b_token_cannot_control_a_ws_a_run() {
    let (gw, key) = gateway().await;
    // The run lives in ws-A.
    seed_run(&gw, "ws-a", "run-x").await;
    // A ws-B token WITH the cap — the control acts in ws-B (the token's ws), where the run does not
    // exist → a 4xx, and ws-A's run is never touched (the hard wall §7).
    let tok_b = token(&key, "user:bob", "ws-b", CONTROL);
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/runs/run-x/pause", serde_json::json!({})),
            &tok_b,
        ))
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "a ws-B control of a ws-A run does not succeed: {}",
        resp.status()
    );
    assert_eq!(
        status(&gw, "ws-a", "run-x").await,
        JobStatus::Running,
        "the ws-A run is untouched — the workspace wall held"
    );
}
