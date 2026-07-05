//! The gateway's `assets.*` routes, end to end (S4 gateway wiring). The workflow-route tests that
//! shared this file were removed with the retired coding workflow (rules-workflow-convergence scope).
//! Each test drives the
//! real routes with `oneshot` (no socket) over a real signed session, and asserts the two properties
//! every gateway surface must hold: a **capability deny** (server-side, the token's caps, never the
//! body) and **workspace isolation** (a ws-B session sees none of ws-A's assets).
//!
//! Shared fixtures in `tests/common/`. Split from `gateway_test.rs` to stay under the FILE-LAYOUT
//! 400-line limit.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::router;
use serde_json::json;
use tower::ServiceExt; // for `oneshot`

// The store caps the asset routes check (capability-first, §3.5) — the route calls the host verb
// directly, so the gate is the `store:doc/*` / `store:skill/*` cap, not the MCP `mcp:assets.*` one.
const DOC_R: &str = "store:doc/*:read";
const DOC_W: &str = "store:doc/*:write";
const SKILL_R: &str = "store:skill/*:read";
const SKILL_W: &str = "store:skill/*:write";

// ----- assets -----------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn put_then_get_own_doc_round_trips_over_the_gateway() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &[DOC_R, DOC_W]);

    // Create a doc owned by the token's principal.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/docs",
                json!({ "id": "spec", "title": "Spec", "content": "hello" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Read it back — the owner may always read their own doc (gate 3 passes by ownership).
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/docs/spec"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let doc: serde_json::Value = json_body(resp).await;
    assert_eq!(doc["content"], "hello");
    assert_eq!(doc["owner"], "user:ada");

    // And it appears in the owner's doc list.
    let resp = router(gw)
        .oneshot(bearer(get_req("/docs"), &tok))
        .await
        .unwrap();
    let docs: Vec<serde_json::Value> = json_body(resp).await;
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0]["id"], "spec");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn put_doc_without_the_write_cap_is_denied_server_side() {
    let (gw, key) = gateway().await;
    // A read-only token — no `store:doc/*:write`. The deny is from the TOKEN's caps, server-side,
    // not the body (the hard wall, §7).
    let tok = token(&key, "user:ada", "acme", &[DOC_R]);

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/docs", json!({ "id": "x", "title": "X", "content": "c" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_session_cannot_read_ws_a_doc() {
    // Two sessions, one node — the real isolation setup. ws-A creates a doc; a ws-B session (its own
    // token, same key) must not see it, even with the doc caps.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.unwrap());
    let key = SigningKey::generate();
    let gw = gateway_on(node.clone(), &key);

    let tok_a = token(&key, "user:ada", "acme", &[DOC_R, DOC_W]);
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/docs",
                json!({ "id": "secret", "title": "S", "content": "x" }),
            ),
            &tok_a,
        ))
        .await
        .unwrap();

    // ws-B: a different workspace in the token. The workspace comes from the token, so this caller
    // queries ws-B — where the doc does not exist → NotFound (it is not even visible to deny).
    let tok_b = token(&key, "user:bob", "other", &[DOC_R, DOC_W]);
    let resp = router(gw)
        .oneshot(bearer(get_req("/docs/secret"), &tok_b))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn grant_then_load_skill_round_trips_and_ungranted_load_is_denied() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &[SKILL_R, SKILL_W]);

    // Put a skill version.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/skills",
                json!({ "id": "summarize", "version": "1", "description": "d", "body": "Be brief." }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Before the grant, loading it is denied (the grant gate the agent inherits — skills scope).
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/skills/summarize"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Grant it to the workspace, then it loads.
    router(gw.clone())
        .oneshot(bearer(
            json_post("/skills/summarize/grant", json!({})),
            &tok,
        ))
        .await
        .unwrap();
    let resp = router(gw)
        .oneshot(bearer(get_req("/skills/summarize"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let s: serde_json::Value = json_body(resp).await;
    assert_eq!(s["body"], "Be brief.");
}
