//! The insights gateway routes over the REAL spawned gateway + real store + real caps (insights
//! umbrella scope). NO mocks (CLAUDE §9): records are seeded by raising through the real verb,
//! then read back through the REST routes.
//!
//! **SKELETON**: every test is NAMED for a mandatory or scope-named case + carries the real-
//! gateway setup boilerplate. Bodies are `todo!()` so a green-but-lying stub is impossible. The
//! implementing session fills them against the scope docs. The harness mirrors
//! `webhook_routes_test.rs` / `inbox` patterns.
//!
//! Mandatory categories: capability-deny (per route) + workspace-isolation. The matcher/ladder/
//! digest surfaces are exercised through the MCP bridge (`POST /mcp/call`) — their pure unit
//! tests live in the insights crate; their integration headlines live in `host/tests/insights_test.rs`.

mod common;

use axum::http::StatusCode;
use common::{bearer, gateway, get_req, json_body, json_post, token};
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt;

/// The full insight MCP surface caps a member holds (mirrors dev-login).
const MEMBER_CAPS: &[&str] = &[
    "mcp:insight.raise:call",
    "mcp:insight.get:call",
    "mcp:insight.list:call",
    "mcp:insight.ack:call",
    "mcp:insight.resolve:call",
    "mcp:insight.occurrences:call",
    "mcp:insight.sub.create:call",
    "mcp:insight.sub.list:call",
    "mcp:insight.sub.get:call",
    "mcp:insight.sub.delete:call",
    "mcp:insight.sub.mute:call",
    "mcp:insight.policy.get:call",
    "bus:chan/*:pub",
];

/// Mint a member JWT for `ws`.
fn member(key: &lb_auth::SigningKey, sub: &str, ws: &str) -> String {
    token(key, sub, ws, MEMBER_CAPS)
}

/// POST /mcp/call helper to drive an `insight.*` verb over the universal bridge.
async fn mcp(app: &axum::Router, tok: &str, tool: &str, args: Value) -> Value {
    let body = json!({ "tool": tool, "args": args });
    let resp = app
        .clone()
        .oneshot(bearer(json_post("/mcp/call", body), tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "mcp {tool} failed");
    json_body(resp).await
}

// --- mandatory: capability deny ----------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_denied_without_the_cap() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = token(&key, "user:bob", "acme", &["mcp:insight.get:call"]); // no LIST
    let r = app
        .clone()
        .oneshot(bearer(get_req("/insights"), &tok))
        .await
        .unwrap();
    // Deny is an opaque 403 — a caller without the cap can't tell empty from forbidden.
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ack_denied_without_the_cap() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = token(
        &key,
        "user:bob",
        "acme",
        &["mcp:insight.raise:call", "mcp:insight.get:call"], // no ACK
    );
    let r = app
        .clone()
        .oneshot(bearer(json_post("/insights/ins-1/ack", json!({})), &tok))
        .await
        .unwrap();
    // Ack without the cap is denied opaque (403) — before any record is read.
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

// --- mandatory: workspace isolation ------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cross_workspace_insight_is_opaque_to_the_other_ws() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let a = member(&key, "user:ada", "ws-a");
    let b = member(&key, "user:bea", "ws-b");
    let _ = mcp(
        &app,
        &a,
        "insight.raise",
        json!({ "dedup_key": "k1", "severity": "critical", "title": "ws-a only",
                "origin": { "kind": "manual", "ref": "test" }, "ts": 1 }),
    )
    .await;
    let b_list = app
        .clone()
        .oneshot(bearer(get_req("/insights"), &b))
        .await
        .unwrap();
    assert_eq!(b_list.status(), StatusCode::OK);
    let body: Value = json_body(b_list).await;
    assert_eq!(
        body["items"].as_array().unwrap().len(),
        0,
        "ws-B's list never surfaces ws-A's insight"
    );
    // And ws-A does see its own (sanity that the raise landed).
    let a_list = app
        .clone()
        .oneshot(bearer(get_req("/insights"), &a))
        .await
        .unwrap();
    let a_body: Value = json_body(a_list).await;
    assert_eq!(a_body["items"].as_array().unwrap().len(), 1);
}

// --- the headline round-trip (raise → list → get → ack → resolve) ------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn raise_list_get_ack_resolve_round_trip() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = member(&key, "user:ada", "ws-happy");

    // raise over the MCP bridge
    let _raised = mcp(
        &app,
        &tok,
        "insight.raise",
        json!({ "dedup_key": "k1", "severity": "warning", "title": "headline",
                "origin": { "kind": "manual", "ref": "test" },
                "occurrence": { "data": { "score": 0.71 } }, "ts": 1 }),
    )
    .await;

    // list over the REST route — the insight is there, open.
    let listed = app
        .clone()
        .oneshot(bearer(get_req("/insights"), &tok))
        .await
        .unwrap();
    let list_body: Value = json_body(listed).await;
    let items = list_body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    let id = items[0]["id"].as_str().unwrap().to_string();
    assert_eq!(items[0]["status"], "open");

    // get by id over the REST route.
    let got = app
        .clone()
        .oneshot(bearer(get_req(&format!("/insights/{id}")), &tok))
        .await
        .unwrap();
    let got_body: Value = json_body(got).await;
    assert_eq!(got_body["dedup_key"], "k1");

    // ack over the REST route (the client passes no `now` — the gateway stamps it).
    let acked = app
        .clone()
        .oneshot(bearer(
            json_post(&format!("/insights/{id}/ack"), json!({})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(acked.status(), StatusCode::NO_CONTENT);

    // resolve with a note.
    let resolved = app
        .clone()
        .oneshot(bearer(
            json_post(
                &format!("/insights/{id}/resolve"),
                json!({ "note": "handled" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resolved.status(), StatusCode::NO_CONTENT);

    // the lifecycle landed: get now shows resolved.
    let final_get = app
        .clone()
        .oneshot(bearer(get_req(&format!("/insights/{id}")), &tok))
        .await
        .unwrap();
    let final_body: Value = json_body(final_get).await;
    assert_eq!(final_body["status"], "resolved");

    // occurrences round-trip too (one row from the raise's occurrence).
    let occ = app
        .clone()
        .oneshot(bearer(
            get_req(&format!("/insights/{id}/occurrences")),
            &tok,
        ))
        .await
        .unwrap();
    let occ_body: Value = json_body(occ).await;
    assert_eq!(occ_body["items"].as_array().unwrap().len(), 1);
}
