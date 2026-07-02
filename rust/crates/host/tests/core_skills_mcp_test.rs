//! The core-skills MCP surface over `call_asset_tool` (README §6.5 — one MCP contract). Every verb
//! the slice added or touched is reachable AND deny-tested at the MCP gate (per-verb deny, the
//! HOW-TO-CODE §3 step 4a requirement): `list_skills` (the agent catalog with tier rows),
//! `deprecate_skill`, `revoke_skill`, plus the reserved-namespace rejection through the bridge.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_asset_tool, seed_core_skills, Node};
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

// The skill surface caps use `**` (recursive tail) — a core id contains a `.`, and the grammar
// splits a resource on `/` AND `.`, so `skill/*` would not cover `skill/core.lb-cli`.
const READ: &str = "store:skill/**:read";
const WRITE: &str = "store:skill/**:write";

/// The full per-verb MCP cap set the caller holds for the happy path.
fn full(ws: &str) -> Principal {
    principal(
        "user:ada",
        ws,
        &[
            "mcp:assets.list_skills:call",
            "mcp:assets.put_skill:call",
            "mcp:assets.grant_skill:call",
            "mcp:assets.revoke_skill:call",
            "mcp:assets.deprecate_skill:call",
            "mcp:assets.load_skill:call",
            READ,
            WRITE,
        ],
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_skills_over_the_bridge_carries_tier_rows() {
    let ws = "ws-mcp-list-skills";
    let node = Node::boot().await.unwrap();
    seed_core_skills(&node.store, "0.1.0", 1).await.unwrap();
    let ada = full(ws);

    // Author + grant a user skill; grant a core skill.
    call_asset_tool(
        &node.store,
        &ada,
        ws,
        "assets.put_skill",
        &json!({"id":"acme-runbook","version":"1.0.0","description":"the runbook","body":"b","ts":1}),
    )
    .await
    .unwrap();
    call_asset_tool(
        &node.store,
        &ada,
        ws,
        "assets.grant_skill",
        &json!({"id":"acme-runbook"}),
    )
    .await
    .unwrap();
    call_asset_tool(
        &node.store,
        &ada,
        ws,
        "assets.grant_skill",
        &json!({"id":"core.lb-cli"}),
    )
    .await
    .unwrap();

    let out = call_asset_tool(&node.store, &ada, ws, "assets.list_skills", &json!({}))
        .await
        .unwrap();
    let skills = out["skills"].as_array().unwrap();
    let core = skills.iter().find(|s| s["id"] == "core.lb-cli").unwrap();
    assert_eq!(core["tier"], "core");
    assert_eq!(core["granted"], true);
    assert!(core["description"].as_str().unwrap().len() > 0);
    let user = skills.iter().find(|s| s["id"] == "acme-runbook").unwrap();
    assert_eq!(user["tier"], "user");
    assert_eq!(user["description"], "the runbook");
    // No body field leaks into the catalog rows.
    assert!(core.get("body").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deprecate_and_revoke_over_the_bridge() {
    let ws = "ws-mcp-deprecate";
    let node = Node::boot().await.unwrap();
    let ada = full(ws);

    call_asset_tool(
        &node.store,
        &ada,
        ws,
        "assets.put_skill",
        &json!({"id":"r","version":"1.0.0","description":"d","body":"b","ts":1}),
    )
    .await
    .unwrap();
    call_asset_tool(
        &node.store,
        &ada,
        ws,
        "assets.grant_skill",
        &json!({"id":"r"}),
    )
    .await
    .unwrap();
    // In the catalog.
    let out = call_asset_tool(&node.store, &ada, ws, "assets.list_skills", &json!({}))
        .await
        .unwrap();
    assert_eq!(out["skills"].as_array().unwrap().len(), 1);

    // Deprecate over the bridge → gone from the catalog.
    call_asset_tool(
        &node.store,
        &ada,
        ws,
        "assets.deprecate_skill",
        &json!({"id":"r"}),
    )
    .await
    .unwrap();
    let out = call_asset_tool(&node.store, &ada, ws, "assets.list_skills", &json!({}))
        .await
        .unwrap();
    assert!(out["skills"].as_array().unwrap().is_empty());

    // Revoke over the bridge (idempotent, ok).
    call_asset_tool(
        &node.store,
        &ada,
        ws,
        "assets.revoke_skill",
        &json!({"id":"r"}),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn put_core_over_the_bridge_is_a_clear_bad_input_not_an_opaque_deny() {
    let ws = "ws-mcp-core-reserved";
    let node = Node::boot().await.unwrap();
    let ada = full(ws);
    let err = call_asset_tool(
        &node.store,
        &ada,
        ws,
        "assets.put_skill",
        &json!({"id":"core.lb-cli","version":"9.9.9","description":"x","body":"x","ts":1}),
    )
    .await
    .unwrap_err();
    // Reserved is NON-opaque (a public namespace rule), surfaced as BadInput — not Denied.
    assert!(matches!(err, ToolError::BadInput(_)), "got {err:?}");
}

// ── per-verb MCP deny: each verb refused at the MCP gate without its cap ──────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_new_verb_is_denied_at_the_mcp_gate_without_its_cap() {
    let ws = "ws-mcp-deny";
    let node = Node::boot().await.unwrap();
    // Holds the STORE caps but NONE of the mcp:assets.*:call verb caps → refused at the MCP gate.
    let no_mcp = principal("user:ada", ws, &[READ, WRITE]);

    for (verb, args) in [
        ("assets.list_skills", json!({})),
        ("assets.deprecate_skill", json!({"id":"r"})),
        ("assets.revoke_skill", json!({"id":"r"})),
        ("assets.grant_skill", json!({"id":"r"})),
        ("assets.load_skill", json!({"id":"r"})),
        (
            "assets.put_skill",
            json!({"id":"r","version":"1","description":"d","body":"b","ts":1}),
        ),
    ] {
        let err = call_asset_tool(&node.store, &no_mcp, ws, verb, &args)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "{verb} must be denied at the MCP gate, got {err:?}"
        );
    }
}
