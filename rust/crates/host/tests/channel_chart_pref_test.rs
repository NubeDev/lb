//! The per-viewer chart-preference verbs through the REAL MCP bridge (`lb_host::call_tool`) — the
//! same entry the gateway's `POST /mcp/call` forwards. Proves: a member round-trips set→get; the
//! override is PER-USER (one viewer's plot never leaks into another's); a caller missing the MCP grant
//! (or the channel `sub` cap) is denied opaquely; and the workspace is the hard wall (a ws-B caller
//! never reads a ws-A pref). These are the mandatory capability-deny + workspace-isolation tests
//! (testing scope §0 / CLAUDE §5–6).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};
use std::sync::Arc;

/// The MCP grants a channel member holds for the chart-pref surface + the channel `sub` gate the
/// verb re-checks. (Mirrors the gateway `member_caps()`.)
const MEMBER: &[&str] = &[
    "mcp:channel.chart_pref.get:call",
    "mcp:channel.chart_pref.set:call",
    "bus:chan/*:sub",
];

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

fn spec() -> Value {
    json!({ "type": "line", "xField": "t", "yFields": ["cpu", "mem"], "smooth": true })
}

async fn get(node: &Arc<Node>, p: &Principal, ws: &str, chan: &str, item: &str) -> Value {
    let out = call_tool(
        node,
        p,
        ws,
        "channel.chart_pref.get",
        &json!({ "channel": chan, "item": item }).to_string(),
    )
    .await
    .expect("get authorized");
    serde_json::from_str::<Value>(&out).unwrap()["spec"].clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_then_get_round_trips_per_user() {
    let ws = "cp-rt";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, MEMBER);
    let bob = principal("user:bob", ws, MEMBER);

    // Nothing saved yet → null (the surface uses the host default).
    assert_eq!(get(&node, &ada, ws, "general", "q:r1").await, Value::Null);

    // Ada saves her plot; it round-trips back for Ada.
    call_tool(
        &node,
        &ada,
        ws,
        "channel.chart_pref.set",
        &json!({ "channel": "general", "item": "q:r1", "spec": spec() }).to_string(),
    )
    .await
    .expect("set authorized");
    assert_eq!(get(&node, &ada, ws, "general", "q:r1").await, spec());

    // Bob viewing the SAME result has no override of his own — per-user isolation.
    assert_eq!(get(&node, &bob, ws, "general", "q:r1").await, Value::Null);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denied_without_the_grant_is_opaque() {
    let ws = "cp-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // Holds the channel sub cap but NOT the MCP verb grant → the outer gate denies opaquely.
    let ungranted = principal("user:eve", ws, &["bus:chan/*:sub"]);
    let err = call_tool(
        &node,
        &ungranted,
        ws,
        "channel.chart_pref.set",
        &json!({ "channel": "general", "item": "q:r1", "spec": spec() }).to_string(),
    )
    .await
    .expect_err("no mcp grant → denied");
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");

    // Holds the MCP grant but NOT the channel `sub` cap → the verb's inner gate denies.
    let no_sub = principal("user:mallory", ws, &["mcp:channel.chart_pref.get:call"]);
    let err = call_tool(
        &node,
        &no_sub,
        ws,
        "channel.chart_pref.get",
        &json!({ "channel": "general", "item": "q:r1" }).to_string(),
    )
    .await
    .expect_err("no channel sub cap → denied");
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_is_the_hard_wall() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ada_a = principal("user:ada", "ws-a", MEMBER);
    let ada_b = principal("user:ada", "ws-b", MEMBER);

    // Same user, same channel+item, different workspace → the ws-B read never sees the ws-A write.
    call_tool(
        &node,
        &ada_a,
        "ws-a",
        "channel.chart_pref.set",
        &json!({ "channel": "general", "item": "q:r1", "spec": spec() }).to_string(),
    )
    .await
    .expect("set in ws-a");
    assert_eq!(get(&node, &ada_a, "ws-a", "general", "q:r1").await, spec());
    assert_eq!(
        get(&node, &ada_b, "ws-b", "general", "q:r1").await,
        Value::Null
    );
}
