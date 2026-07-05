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

use axum::body::Body;
use axum::http::{Request, StatusCode};
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
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    // SCOPE: insights-scope.md §"How it fits the core" → Capabilities.
    todo!("insights: assert list deny is opaque 403 — SCOPE: insights-scope.md §Capabilities")
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
    let _r = app
        .clone()
        .oneshot(bearer(json_post("/insights/ins-1/ack", json!({})), &tok))
        .await
        .unwrap();
    // SCOPE: insights-scope.md §"How it fits the core" → Capabilities.
    todo!("insights: assert ack deny is opaque 403 — SCOPE: insights-scope.md §Capabilities")
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
    let _b_list = app
        .clone()
        .oneshot(bearer(get_req("/insights"), &b))
        .await
        .unwrap();
    // SCOPE: insights-scope.md §"How it fits the core" → Tenancy/isolation. ws-B's list never
    // surfaces ws-A's insight; the watch subject leaks nothing cross-ws.
    todo!("insights: ws-B /insights returns empty (no leak) — SCOPE: insights-scope.md §Tenancy")
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

    // list over the REST route
    let _listed = app
        .clone()
        .oneshot(bearer(get_req("/insights"), &tok))
        .await
        .unwrap();

    // SCOPE: insights-scope.md §"MCP surface" — the full surface round-trips.
    todo!("insights: raise → list returns it → get by id → ack → resolve; assert the lifecycle transitions")
}
