//! Mandatory tags workspace-isolation at the host/MCP layer (tags scope, testing §2.2). The
//! specified test: construct the IDENTICAL `tag:['region','eu']` in BOTH workspaces, write edges in
//! each, and assert a ws-B find returns ZERO ws-A entities — and a ws-B token aimed at ws-A is denied
//! at gate 1. Using the same tag VALUE in both is deliberate: a test with different values would pass
//! even with a leak, so it is disallowed.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::call_tags_tool;
use lb_mcp::ToolError;
use lb_store::Store;
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

const ALL: &[&str] = &[
    "mcp:tags.add:call",
    "mcp:tags.of:call",
    "mcp:tags.find:call",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn identical_tag_does_not_leak_across_workspaces_via_mcp() {
    let store = Store::memory().await.unwrap();
    let a = principal("user:a", "ws-a", ALL);
    let b = principal("user:b", "ws-b", ALL);

    // The IDENTICAL tag id + key + value, in BOTH workspaces, each on its own entity.
    call_tags_tool(
        &store,
        &a,
        "ws-a",
        "tags.add",
        &json!({ "entity": "series:a", "key": "region", "value": "eu" }),
    )
    .await
    .unwrap();
    call_tags_tool(
        &store,
        &b,
        "ws-b",
        "tags.add",
        &json!({ "entity": "series:b", "key": "region", "value": "eu" }),
    )
    .await
    .unwrap();

    // ws-B find for the identical tag returns ONLY ws-B's entity.
    let b_hits = call_tags_tool(
        &store,
        &b,
        "ws-b",
        "tags.find",
        &json!({ "facets": [{"key": "region", "value": "eu"}] }),
    )
    .await
    .unwrap();
    assert_eq!(
        b_hits["entities"],
        json!(["series:b"]),
        "ws-B sees only its own edge"
    );

    // And ws-A likewise.
    let a_hits = call_tags_tool(
        &store,
        &a,
        "ws-a",
        "tags.find",
        &json!({ "facets": [{"key": "region", "value": "eu"}] }),
    )
    .await
    .unwrap();
    assert_eq!(a_hits["entities"], json!(["series:a"]));

    // A ws-B token aimed at ws-A's namespace is refused at gate 1 (workspace), opaque Denied.
    let cross = call_tags_tool(
        &store,
        &b,
        "ws-a",
        "tags.find",
        &json!({ "facets": [{"key": "region", "value": "eu"}] }),
    )
    .await
    .unwrap_err();
    assert!(matches!(cross, ToolError::Denied));
}
