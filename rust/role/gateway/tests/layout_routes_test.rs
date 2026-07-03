//! The `layout.*` gateway routes end to end (data-studio scope v2, "Layout persistence"): the
//! member-owned per-surface workbench layout over `GET/PUT /layout/{surface}`. Proves the round-trip,
//! the member-owned keying via the token `sub` (a second user on the same surface sees their own,
//! absent layout — never the first user's), the capability deny per verb, and workspace isolation.
//! The workspace + owner come from the TOKEN, never the body (§7) — this is the boundary the UI
//! cap-gate only mirrors.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use lb_role_gateway::router;
use tower::ServiceExt; // for `oneshot`

const GET: &str = "mcp:layout.get:call";
const SET: &str = "mcp:layout.set:call";

fn json_put(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn round_trip_and_member_owned_keying() {
    let (gw, key) = gateway().await;
    let ada = token(&key, "user:ada", "acme", &[GET, SET]);
    let ben = token(&key, "user:ben", "acme", &[GET, SET]);

    // Ada saves a layout.
    let model = serde_json::json!({ "layout": { "type": "row" }, "tabs": ["explore-1"] });
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_put("/layout/data-studio", serde_json::json!({ "model": model })),
            &ada,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Ada reads it back.
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/layout/data-studio"), &ada))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let got: serde_json::Value = json_body(resp).await;
    assert_eq!(got["model"], model);
    assert_eq!(got["surface"], "data-studio");

    // Ben, SAME workspace + SAME surface, reads HIS OWN (absent) layout — never Ada's. The record is
    // keyed to the token `sub`; there is no body field through which Ben could name Ada.
    let resp = router(gw)
        .oneshot(bearer(get_req("/layout/data-studio"), &ben))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bens: serde_json::Value = json_body(resp).await;
    assert!(
        bens["model"].is_null(),
        "ben sees his own empty layout, not ada's"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn capability_deny_per_verb() {
    let (gw, key) = gateway().await;
    let only_get = token(&key, "user:ada", "acme", &[GET]);
    let only_set = token(&key, "user:ben", "acme", &[SET]);

    // Missing `layout.set` → PUT is 403 (opaque), even with the read grant.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_put("/layout/data-studio", serde_json::json!({ "model": {} })),
            &only_get,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Missing `layout.get` → GET is 403, even with the write grant.
    let resp = router(gw)
        .oneshot(bearer(get_req("/layout/data-studio"), &only_set))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation() {
    let (gw, key) = gateway().await;
    let in_a = token(&key, "user:ada", "acme", &[GET, SET]);
    let in_b = token(&key, "user:ada", "beta", &[GET, SET]);

    router(gw.clone())
        .oneshot(bearer(
            json_put(
                "/layout/data-studio",
                serde_json::json!({ "model": { "ws": "a" } }),
            ),
            &in_a,
        ))
        .await
        .unwrap();

    // Same user, different workspace — the hard wall: nothing crosses.
    let resp = router(gw)
        .oneshot(bearer(get_req("/layout/data-studio"), &in_b))
        .await
        .unwrap();
    let got: serde_json::Value = json_body(resp).await;
    assert!(got["model"].is_null(), "ws-B sees no layout from ws-A");
}
