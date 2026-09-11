//! The **case plane** — a case is a piece of work that cites insights
//! (`docs/scope/insights/case-plane-scope.md`), over a REAL booted `Node`. Real store, real bus,
//! real caps, the real `call_tool` MCP bridge. NO mocks (CLAUDE §9): every record is created by
//! calling the verb under test and read back through it.
//!
//! Mandatory categories, per verb: **capability-deny** (a principal without the cap → `Denied`) and
//! **workspace-isolation** (a ws-B principal cannot see or touch a ws-A case).
//!
//! Plus the scope's named cases: the resolution invariant, the exclusivity invariant, workflow
//! immunity under 50 re-raises, the `Mine` lane resolving through TEAMS, and — the one that catches
//! the mistake this table exists to prevent — a POSITIVE gate test per aliased verb, because a
//! missing `tool_gate.rs` arm is `Denied`, not `NotFound`.
//!
//! **Fixtures only.** Split out of `case_plane_test.rs` when that file passed the 400-line
//! FILE-LAYOUT limit. The tests live in the `plane_*.rs` siblings, aggregated by
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
pub const I_ASSIGN: &str = "mcp:insight.assign:call";
pub const I_COMMENT: &str = "mcp:insight.comment:call";
pub const GET: &str = "mcp:case.get:call";
pub const LIST: &str = "mcp:case.list:call";
pub const OPEN: &str = "mcp:case.open:call";
pub const WORKFLOW: &str = "mcp:case.workflow:call";

/// Every case cap plus the insight caps a test needs to seed with — the "fully-empowered operator".
pub const ALL: &[&str] = &[RAISE, I_GET, I_ASSIGN, I_COMMENT, GET, LIST, OPEN, WORKFLOW];

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

/// Raise one insight and return its id. Grouping runs inline, so this also mints its case.
pub async fn seed_insight(node: &Arc<Node>, p: &Principal, ws: &str, key: &str, ts: u64) -> String {
    let out = call(node, p, ws, "insight.raise", raise_input(key, ts))
        .await
        .expect("raise ok");
    out["id"].as_str().unwrap().to_string()
}

/// The case the grouping reactor opened for `insight_id` — read back through `insight.get`'s echo,
/// which is the same path a roster uses.
pub async fn case_of(node: &Arc<Node>, p: &Principal, ws: &str, insight_id: &str) -> String {
    let out = call(node, p, ws, "insight.get", json!({ "id": insight_id }))
        .await
        .expect("get ok");
    out["case_id"]
        .as_str()
        .unwrap_or_else(|| panic!("insight {insight_id} has no case_id echo: {out}"))
        .to_string()
}

/// A real workspace roster: `user:test` and `user:priya` are members, `team:mechanical` exists with
/// priya on it. Real rows through the real writers — no fixtures (CLAUDE §9).
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
