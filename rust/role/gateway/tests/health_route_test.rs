//! `GET /health` — the fleet health probe, end to end (issue #72). The route is UNAUTHENTICATED
//! (an LB has no bearer token — same posture as `POST /login`), reads in-memory state only, and
//! answers `200`/`503` per the contract decided in `docs/scope/deploy/containerize-scope.md`
//! §"The health contract". These run against the REAL gateway over a real booted node (rule 9 — no
//! fake backend): the 200 shape + version, the open auth posture, the `/healthz` non-registration,
//! the leaks-nothing body, and the 503 degraded path through the in-memory gate. No store query is
//! made on any of these — the probe reads only the `HealthGate` atomics.

mod common;

use axum::http::{Request, StatusCode};
use common::*;
use lb_role_gateway::router;
use tower::ServiceExt; // for `oneshot`

/// The body's top-level key set is exactly `{status, version, detail}` and `detail` is exactly
/// `{store, gateway}` — the contract shape, and nothing a probe can use as an oracle beyond it.
fn assert_leaks_nothing(body: &serde_json::Value, status: &str) {
    let obj = body.as_object().expect("body is an object");
    assert_eq!(
        obj.len(),
        3,
        "only status/version/detail at status={status}"
    );
    assert_eq!(obj["status"], status);
    assert_eq!(obj["version"], env!("CARGO_PKG_VERSION"));
    let detail = obj["detail"].as_object().expect("detail is an object");
    assert_eq!(
        detail.len(),
        2,
        "detail is only store/gateway at status={status}"
    );
    // Values are strictly "ok" | "degraded" — never a path, DSN, or key.
    for (_, v) in detail {
        let s = v.as_str().expect("detail value is a string");
        assert!(
            s == "ok" || s == "degraded",
            "detail value {s:?} not ok/degraded at status={status}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn health_ok_unauthenticated_with_version_and_detail() {
    let (gw, _key) = gateway().await;
    // Bare GET, NO Authorization header — the route sits outside the auth wall.
    let resp = router(gw).oneshot(get_req("/health")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = json_body(resp).await;
    assert_eq!(body["detail"]["store"], "ok");
    assert_eq!(body["detail"]["gateway"], "ok");
    assert_leaks_nothing(&body, "ok");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn health_a_stale_bearer_does_not_change_the_answer() {
    // An LB has no bearer, but the route must also not behave differently when a garbage/expired
    // token IS presented — it never reaches the auth wall.
    let (gw, _key) = gateway().await;
    let req = Request::builder()
        .method("GET")
        .uri("/health")
        .header("authorization", "Bearer not-a-real-token")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = router(gw).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn healthz_is_not_registered() {
    // `/health`, never `/healthz` (the contract). A probe to the wrong spelling 404s, not 200.
    let (gw, _key) = gateway().await;
    let resp = router(gw).oneshot(get_req("/healthz")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    // And the other k8s-ism spellings are not registered either.
    for path in ["/livez", "/readyz", "/startupz", "/api/health"] {
        let resp = router(gateway().await.0)
            .oneshot(get_req(path))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{path} should 404");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn health_503_when_a_subsystem_is_degraded() {
    let (gw, _key) = gateway().await;
    // Flip the in-memory degrade seam the route reads (no store call made).
    gw.health.set_store(false);
    let resp = router(gw).oneshot(get_req("/health")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = json_body(resp).await;
    assert_eq!(body["status"], "degraded");
    assert_eq!(body["detail"]["store"], "degraded");
    assert_eq!(body["detail"]["gateway"], "ok");
    assert_leaks_nothing(&body, "degraded");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn health_503_when_the_gateway_subsystem_is_degraded() {
    let (gw, _key) = gateway().await;
    gw.health.set_gateway(false);
    let resp = router(gw).oneshot(get_req("/health")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = json_body(resp).await;
    assert_eq!(body["detail"]["store"], "ok");
    assert_eq!(body["detail"]["gateway"], "degraded");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn health_returns_to_ok_after_a_degrade_is_cleared() {
    let (gw, _key) = gateway().await;
    gw.health.set_store(false);
    let resp = router(gw.clone())
        .oneshot(get_req("/health"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    gw.health.set_store(true); // a future monitor clears the degrade
    let resp = router(gw).oneshot(get_req("/health")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
