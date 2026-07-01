//! Regression: the retired `/chains…` gateway routes are GONE (chains-retirement scope). The chain
//! DAG surface was removed in favour of `/flows…` (the one DAG engine). A browser still pointing at
//! `/chains` must get a transport 404 — the route is unregistered, not merely cap-gated — while the
//! sibling `/flows` route it was replaced by still answers. Real gateway over a real node, a real
//! session token; no mocks.

mod common;

use axum::http::StatusCode;
use common::*;
use lb_role_gateway::router;
use tower::ServiceExt; // for `oneshot`

/// The four retired `/chains…` routes (method + uri). Each must 404 at the router (unregistered),
/// regardless of caps — the token below carries the full flows surface, proving it's the ROUTE that's
/// gone, not authorization.
const RETIRED_ROUTES: &[(&str, &str)] = &[
    ("GET", "/chains"),
    ("POST", "/chains"),
    ("GET", "/chains/c"),
    ("DELETE", "/chains/c"),
    ("POST", "/chains/c/run"),
    ("GET", "/chains/c/runs/r"),
];

fn req(method: &str, uri: &str) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(axum::body::Body::from("{}"))
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_retired_chains_route_is_404_while_flows_answers() {
    let (gw, key) = gateway().await;
    // A well-provisioned session (the flows surface + store grants) — so a 404 can only mean the
    // ROUTE is gone, never a missing cap.
    let tok = token(
        &key,
        "user:ada",
        "acme",
        &[
            "mcp:flows.list:call",
            "mcp:flows.save:call",
            "store:flow:read",
            "store:flow:write",
        ],
    );

    for (method, uri) in RETIRED_ROUTES {
        let resp = router(gw.clone())
            .oneshot(bearer(req(method, uri), &tok))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "{method} {uri} must be an unregistered-route 404 (chains retired)"
        );
    }

    // The replacement surface still answers (200 with the roster) — the DAG engine moved to /flows,
    // it wasn't deleted wholesale.
    let flows = router(gw)
        .oneshot(bearer(get_req("/flows"), &tok))
        .await
        .unwrap();
    assert_eq!(
        flows.status(),
        StatusCode::OK,
        "/flows (the replacement DAG surface) still answers"
    );
}
