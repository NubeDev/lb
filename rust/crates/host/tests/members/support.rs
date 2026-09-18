//! Shared fixtures for the **members** suite — the team-membership verbs over the real MCP bridge.
//!
//! Real booted `Node`, real store, real caps, the real `call_tool` bridge. NO mocks (CLAUDE §9).
//!
//! **Why this suite exists.** `members.add`/`list`/`remove` shipped with their caps in the role
//! bundles, their authorization written and unit-tested, and NO dispatch arm — so every call over
//! MCP answered `no such tool`. The service tests never caught it because they called the Rust
//! functions directly, and the catalog test only asserted catalog-covers-dispatch, never the
//! reverse. The damage surfaced in the assign picker: `case.assignees` walks the `member` edge to
//! answer "who do I share a team with?", nothing could WRITE that edge over MCP, so the roster was
//! empty on every node and the control offered only *Assign to me*.
//!
//! So the load-bearing assertion across these files is not "add works" — it is that these verbs are
//! reachable **through the bridge a client actually uses**, and that the picker agrees with the
//! assign it exists to perform.
//!
//! **Fixtures only.** The tests live in the sibling files, aggregated by `members_suite.rs`; this
//! module holds what they share and asserts nothing itself.

#![allow(dead_code)]

pub use std::sync::Arc;

pub use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
pub use lb_authz::{membership_add_raw, team_create};
pub use lb_host::{call_tool, Node};
pub use lb_mcp::ToolError;
pub use serde_json::{json, Value};

pub const ADD: &str = "mcp:members.add:call";
pub const M_LIST: &str = "mcp:members.list:call";
pub const T_MANAGE: &str = "mcp:teams.manage:call";
pub const C_LIST: &str = "mcp:case.list:call";
pub const OPEN: &str = "mcp:case.open:call";
pub const WORKFLOW: &str = "mcp:case.workflow:call";
pub const RAISE: &str = "mcp:insight.raise:call";
pub const I_GET: &str = "mcp:insight.get:call";

pub fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

pub async fn call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

/// Two workspace members and an empty team — the state every workspace was stuck in, because the
/// verb that puts somebody ON the team was unreachable.
pub async fn seed(node: &Arc<Node>, ws: &str) {
    membership_add_raw(&node.store, ws, "user:test", 1)
        .await
        .expect("test joins");
    membership_add_raw(&node.store, ws, "user:priya", 1)
        .await
        .expect("priya joins");
    team_create(&node.store, ws, "team:mechanical", "Mechanical crew")
        .await
        .expect("team created");
}

/// Raise one insight and return the case the grouping reactor opened for it — the only way to get a
/// real case id, since a case is minted by the plane rather than written directly.
pub async fn seed_case(node: &Arc<Node>, p: &Principal, ws: &str) -> String {
    let raised = call(
        node,
        p,
        ws,
        "insight.raise",
        json!({
            "dedup_key": "members-picker-probe",
            "severity": "warning",
            "title": "finding for the picker round trip",
            "origin": { "kind": "rule", "ref": "rule:probe" },
            "ts": 1,
        }),
    )
    .await
    .expect("raise ok");
    let insight = raised["id"].as_str().expect("raised id");
    let got = call(node, p, ws, "insight.get", json!({ "id": insight }))
        .await
        .expect("get ok");
    got["case_id"]
        .as_str()
        .unwrap_or_else(|| panic!("insight {insight} has no case_id echo: {got}"))
        .to_string()
}
