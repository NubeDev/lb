//! Asset verbs over the MCP contract (README §6.5, "MCP is the universal contract") — the
//! store+MCP half of the mandatory categories the S4 prompt requires.
//!
//! `call_asset_tool` is the host-native MCP bridge: it runs the MCP authorize gate
//! (`mcp:assets.<verb>:call`, workspace-first) THEN delegates to the asset verb (which adds its
//! own `store:*` + membership/grant gate). Two independent surfaces, both enforced.
//!
//! Tests:
//!   - happy path: put → share → get over the bridge (with both mcp + store caps);
//!   - **MCP deny** (mandatory §2.1): missing `mcp:assets.*:call` → refused at the MCP gate;
//!   - **store deny through MCP**: holds the mcp cap but not the store cap → still refused;
//!   - **MCP isolation** (mandatory §2.2): a ws-B caller cannot reach a ws-A doc through the
//!     bridge (gate 1 fires on the calling side).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_asset_tool, Node};
use lb_mcp::ToolError;
use serde_json::json;

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

const MCP: &str = "mcp:assets.*:call";
const DREAD: &str = "store:doc/*:read";
const DWRITE: &str = "store:doc/*:write";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn put_then_get_over_the_mcp_bridge() {
    let ws = "ws-mcp-roundtrip";
    let node = Node::boot().await.unwrap();
    let ada = principal("user:ada", ws, &[MCP, DREAD, DWRITE]);

    let put = call_asset_tool(
        &node.store,
        &ada,
        ws,
        "assets.put_doc",
        &json!({"id": "scope-x", "title": "Scope X", "content": "draft", "ts": 1}),
    )
    .await
    .unwrap();
    assert_eq!(put["id"], "scope-x");

    let got = call_asset_tool(
        &node.store,
        &ada,
        ws,
        "assets.get_doc",
        &json!({"id": "scope-x"}),
    )
    .await
    .unwrap();
    assert_eq!(got["content"], "draft");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn mcp_gate_denies_without_the_assets_call_cap() {
    let ws = "ws-mcp-deny";
    let node = Node::boot().await.unwrap();
    // Holds the STORE caps but NOT mcp:assets.*:call → refused at the MCP gate, before the verb.
    let ada = principal("user:ada", ws, &[DREAD, DWRITE]);
    let err = call_asset_tool(
        &node.store,
        &ada,
        ws,
        "assets.put_doc",
        &json!({"id": "x", "title": "T", "content": "c", "ts": 1}),
    )
    .await
    .unwrap_err();
    assert_eq!(err, ToolError::Denied);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn store_gate_denies_through_mcp_without_the_store_cap() {
    let ws = "ws-mcp-storedeny";
    let node = Node::boot().await.unwrap();
    // Passes the MCP gate (has mcp:assets.*:call) but lacks the store write cap → the asset gate
    // refuses. An MCP grant never bypasses the store surface.
    let ada = principal("user:ada", ws, &[MCP]);
    let err = call_asset_tool(
        &node.store,
        &ada,
        ws,
        "assets.put_doc",
        &json!({"id": "x", "title": "T", "content": "c", "ts": 1}),
    )
    .await
    .unwrap_err();
    assert_eq!(err, ToolError::Denied);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn mcp_isolation_ws_b_cannot_reach_ws_a_doc() {
    let node = Node::boot().await.unwrap();
    let ada_a = principal("user:ada", "ws-mcp-iso-a", &[MCP, DREAD, DWRITE]);
    call_asset_tool(
        &node.store,
        &ada_a,
        "ws-mcp-iso-a",
        "assets.put_doc",
        &json!({"id": "scope-x", "title": "T", "content": "secret", "ts": 1}),
    )
    .await
    .unwrap();

    // A ws-B token tries to reach into ws-A through the bridge → gate 1 (workspace) refuses.
    let ada_b = principal("user:ada", "ws-mcp-iso-b", &[MCP, DREAD]);
    let err = call_asset_tool(
        &node.store,
        &ada_b,
        "ws-mcp-iso-a", // target another workspace
        "assets.get_doc",
        &json!({"id": "scope-x"}),
    )
    .await
    .unwrap_err();
    assert_eq!(err, ToolError::Denied);
}
