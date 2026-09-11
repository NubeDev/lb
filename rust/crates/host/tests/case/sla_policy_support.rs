//! The **SLA policy plane** — `policy.sla.set` / `policy.sla.list` — over a REAL booted `Node`
//! (`docs/scope/insights/case-plane-scope.md`, the `service_policy` row). Real store (`mem://`),
//! real caps, the real `call_tool` MCP bridge. NO mocks (CLAUDE §4): every policy is written
//! through the verb under test and read back through the other one.
//!
//! Mandatory categories, per verb: **capability-deny** (including the property only the OUTER gate
//! has — a denied caller cannot tell a real id from a fictional one) and **workspace isolation**.
//!
//! Beyond the mandatory two: the list's *order* is asserted, because that order IS the resolution
//! ladder the sla-clock reactor applies — an admin who cannot read the precedence off the page has
//! to read the code instead.
//!
//! **Fixtures only.** Split out of `sla_policy_test.rs` when that file passed the 400-line
//! FILE-LAYOUT limit. The tests live in the `sla_policy_*.rs` siblings, aggregated by
//! `case_suite.rs`; this module holds what they share and asserts nothing itself.

#![allow(dead_code)]

pub use std::sync::Arc;

pub use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
pub use lb_host::{call_tool, member_role_caps, viewer_role_caps, workspace_admin_role_caps, Node};
pub use lb_mcp::ToolError;
pub use serde_json::{json, Value};

pub const SET: &str = "mcp:policy.sla.set:call";
pub const LIST: &str = "mcp:policy.sla.list:call";

/// The admin token most cases use.
pub const ADMIN: &[&str] = &[SET, LIST];

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

/// A policy payload. `match` axes are opaque strings supplied as data — rule 10: no verb, no cap
/// and no test here knows what a category *value* means.
pub fn policy(id: &str, m: Value) -> Value {
    json!({
        "id": id,
        "name": id,
        "match": m,
        "respond_h": 4,
        "resolve_h": 24,
    })
}

/// The ids `policy.sla.list` returned, in the order it returned them.
pub fn ids(out: &Value) -> Vec<String> {
    out.as_array()
        .or_else(|| out.get("policies").and_then(Value::as_array))
        .expect("a list of policies")
        .iter()
        .map(|p| p["id"].as_str().expect("an id").to_string())
        .collect()
}

pub async fn seed(node: &Arc<Node>, p: &Principal, ws: &str, id: &str, m: Value) {
    call(node, p, ws, "policy.sla.set", policy(id, m))
        .await
        .unwrap_or_else(|e| panic!("set {id} in {ws}: {e:?}"));
}
