//! `insight.assign` / `insight.comment` as **delegates** of the case that owns the work
//! (`docs/scope/insights/case-plane-scope.md`, resolved decision 5), over a REAL booted `Node`.
//!
//! The thing under test is a promise about compatibility, not a new feature: the shipped verb
//! surface, the caps and the return shapes must be **exactly** what they were, while the durable
//! answer moves to the case. So every assertion here is either "the old contract still holds" or
//! "the fact landed on the case".
//!
//! The one behaviour that legitimately changed — and it is the point of the plane — is that
//! assigning ONE detection of a fault assigns every detection its case cites. Three symptoms of one
//! chiller fault are one job with one owner, not three people's work. That is asserted explicitly
//! rather than left to be discovered.
//!
//! **Fixtures only.** Split out of `case_delegation_test.rs` when that file passed the 400-line
//! FILE-LAYOUT limit. The tests live in the `delegation_*.rs` siblings, aggregated by
//! `case_suite.rs`; this module holds what they share and asserts nothing itself.

#![allow(dead_code)]

pub use std::sync::Arc;

pub use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
pub use lb_authz::{membership_add_raw, team_create, MEMBER};
pub use lb_host::{call_tool, Node};
pub use lb_mcp::ToolError;
pub use serde_json::{json, Value};

pub const RAISE: &str = "mcp:insight.raise:call";
pub const I_GET: &str = "mcp:insight.get:call";
pub const I_LIST: &str = "mcp:insight.list:call";
pub const I_ASSIGN: &str = "mcp:insight.assign:call";
pub const I_COMMENT: &str = "mcp:insight.comment:call";
pub const GET: &str = "mcp:case.get:call";
pub const LIST: &str = "mcp:case.list:call";

pub const ALL: &[&str] = &[RAISE, I_GET, I_LIST, I_ASSIGN, I_COMMENT, GET, LIST];

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

pub fn raise_input(dedup_key: &str, ts: u64) -> Value {
    json!({
        "dedup_key": dedup_key,
        "severity": "warning",
        "title": format!("finding {dedup_key}"),
        "origin": { "kind": "rule", "ref": "rule:probe" },
        "ts": ts,
    })
}

pub async fn seed_roster(node: &Arc<Node>, ws: &str) {
    membership_add_raw(&node.store, ws, "user:test", 1)
        .await
        .expect("test joins");
    membership_add_raw(&node.store, ws, "user:priya", 1)
        .await
        .expect("priya joins");
    team_create(&node.store, ws, "team:mechanical", "Mechanical crew")
        .await
        .expect("team created");
    lb_assets::relate(&node.store, ws, MEMBER, "team:mechanical", "user:priya")
        .await
        .expect("priya joins the crew");
}

pub async fn seed_insight(node: &Arc<Node>, p: &Principal, ws: &str, key: &str, ts: u64) -> String {
    call(node, p, ws, "insight.raise", raise_input(key, ts))
        .await
        .expect("raise ok")["id"]
        .as_str()
        .unwrap()
        .to_string()
}

pub async fn case_of(node: &Arc<Node>, p: &Principal, ws: &str, insight_id: &str) -> String {
    call(node, p, ws, "insight.get", json!({ "id": insight_id }))
        .await
        .expect("get ok")["case_id"]
        .as_str()
        .unwrap()
        .to_string()
}
