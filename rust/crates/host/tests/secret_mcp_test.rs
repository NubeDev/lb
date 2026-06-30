//! `secret.*` over the MCP contract (secrets scope, README §6.5) — the host-mediated secret
//! surface reached as MCP tools, exercising the three gates through the REAL bridge + REAL store:
//!
//! - happy path: `secret.set` → `secret.get` round-trip for the owner;
//! - **capability-deny** (mandatory): missing `secret:<path>:get` → refused (gate 2);
//! - **ownership-deny** (the load-bearing NEW test): a second extension holding the literal path
//!   grant is DENIED the first's `Private` secret (gate 3, the owner wall) — proven across two
//!   real `ext:` principals through the MCP dispatch, not a unit mock;
//! - **visibility toggle**: owner flips `Private → Workspace` → the sibling reads; back → denied;
//! - **mediation invariant**: `secret.list` returns metadata and NEVER the value;
//! - **MCP gate**: missing `mcp:secret.*:call` → refused at the MCP gate before the secret gate.
//!
//! The two `ext:` principals stand in for the host-stamped `caller ∩ install-grant` identities
//! (the host sets `owner = principal.sub()`; a guest cannot forge it). Real store, real caps.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn secret_surface_three_gates_over_mcp() {
    let ws = "ws-secret-mcp";
    let node = Arc::new(Node::boot().await.unwrap());

    // The MQTT extension — host-stamped identity `ext:mqtt`, granted its own namespace + the MCP
    // call caps. This is the "real seeded extension install" posture: the principal the host
    // would build at `build_call_context` (`caller ∩ install-grant`).
    let mqtt = principal(
        "ext:mqtt",
        ws,
        &[
            "mcp:secret.set:call",
            "mcp:secret.get:call",
            "mcp:secret.set_visibility:call",
            "mcp:secret.delete:call",
            "mcp:secret.list:call",
            "secret:ext/mqtt/*:write",
            "secret:ext/mqtt/*:get",
            "secret:**:get",
        ],
    );

    // --- happy path: owner set → get ---
    call(
        &node,
        &mqtt,
        ws,
        "secret.set",
        json!({"path": "ext/mqtt/broker-pw", "value": "s3cr3t"}),
    )
    .await
    .unwrap();
    let got = call(
        &node,
        &mqtt,
        ws,
        "secret.get",
        json!({"path": "ext/mqtt/broker-pw"}),
    )
    .await
    .unwrap();
    assert_eq!(got["value"], "s3cr3t");

    // --- capability-deny (gate 2): a caller with the MCP cap but no secret:get grant ---
    let no_get = principal(
        "ext:noread",
        ws,
        &[
            "mcp:secret.get:call",
            "secret:ext/mqtt/*:write", // write only, no :get
        ],
    );
    let err = call(
        &node,
        &no_get,
        ws,
        "secret.get",
        json!({"path": "ext/mqtt/broker-pw"}),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "gate 2 deny: {err:?}");

    // --- MCP gate deny: missing mcp:secret.*:call → refused before the secret gate ---
    let no_mcp = principal("ext:nomcp", ws, &["secret:ext/mqtt/*:get"]);
    let err = call(
        &node,
        &no_mcp,
        ws,
        "secret.get",
        json!({"path": "ext/mqtt/broker-pw"}),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "MCP gate deny: {err:?}");

    // --- ownership-deny (gate 3, the load-bearing test): a second extension with the literal
    //     path grant is DENIED the first's Private secret, through the real MCP dispatch. ---
    let reporting = principal(
        "ext:reporting",
        ws,
        &["mcp:secret.get:call", "secret:ext/mqtt/broker-pw:get"],
    );
    let err = call(
        &node,
        &reporting,
        ws,
        "secret.get",
        json!({"path": "ext/mqtt/broker-pw"}),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "non-owner with the cap is denied a Private secret (owner wall): {err:?}"
    );

    // --- visibility toggle: owner flips to Workspace → sibling now reads; back → denied ---
    call(
        &node,
        &mqtt,
        ws,
        "secret.set_visibility",
        json!({"path": "ext/mqtt/broker-pw", "visibility": "workspace"}),
    )
    .await
    .unwrap();
    let shared = call(
        &node,
        &reporting,
        ws,
        "secret.get",
        json!({"path": "ext/mqtt/broker-pw"}),
    )
    .await
    .unwrap();
    assert_eq!(shared["value"], "s3cr3t", "sibling reads once Workspace-shared");

    call(
        &node,
        &mqtt,
        ws,
        "secret.set_visibility",
        json!({"path": "ext/mqtt/broker-pw", "visibility": "private"}),
    )
    .await
    .unwrap();
    let err = call(
        &node,
        &reporting,
        ws,
        "secret.get",
        json!({"path": "ext/mqtt/broker-pw"}),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "flipped back to Private");

    // Only the owner may toggle: the sibling holds the write cap on the path but is not the owner.
    let reporting_write = principal(
        "ext:reporting",
        ws,
        &[
            "mcp:secret.set_visibility:call",
            "secret:ext/mqtt/broker-pw:write",
        ],
    );
    let err = call(
        &node,
        &reporting_write,
        ws,
        "secret.set_visibility",
        json!({"path": "ext/mqtt/broker-pw", "visibility": "workspace"}),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "only owner toggles");

    // --- mediation invariant: secret.list returns metadata, NEVER the value ---
    let listed = call(&node, &mqtt, ws, "secret.list", json!({}))
        .await
        .unwrap();
    let dumped = listed.to_string();
    assert!(
        !dumped.contains("s3cr3t"),
        "secret.list LEAKED the value: {dumped}"
    );
    let entries = listed["secrets"].as_array().unwrap();
    assert!(entries.iter().any(|e| e["path"] == "ext/mqtt/broker-pw"));
    assert!(entries.iter().all(|e| e.get("value").is_none()));

    // --- owner delete works; the secret is then gone ---
    call(
        &node,
        &mqtt,
        ws,
        "secret.delete",
        json!({"path": "ext/mqtt/broker-pw"}),
    )
    .await
    .unwrap();
    let err = call(
        &node,
        &mqtt,
        ws,
        "secret.get",
        json!({"path": "ext/mqtt/broker-pw"}),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(_)),
        "deleted secret is not found: {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_across_the_bridge() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws_a = "ws-secret-iso-a";
    let ws_b = "ws-secret-iso-b";

    let a = principal(
        "ext:mqtt",
        ws_a,
        &[
            "mcp:secret.set:call",
            "mcp:secret.get:call",
            "secret:ext/mqtt/*:write",
            "secret:ext/mqtt/*:get",
        ],
    );
    call(
        &node,
        &a,
        ws_a,
        "secret.set",
        json!({"path": "ext/mqtt/broker-pw", "value": "ws-a-secret"}),
    )
    .await
    .unwrap();

    // ws-B caller with identical caps cannot see ws-A's secret (gate 1 — the namespace wall).
    let b = principal(
        "ext:mqtt",
        ws_b,
        &[
            "mcp:secret.get:call",
            "secret:ext/mqtt/*:write",
            "secret:ext/mqtt/*:get",
        ],
    );
    let err = call(
        &node,
        &b,
        ws_b,
        "secret.get",
        json!({"path": "ext/mqtt/broker-pw"}),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied | ToolError::BadInput(_)),
        "ws-B cannot read ws-A's secret: {err:?}"
    );
}
