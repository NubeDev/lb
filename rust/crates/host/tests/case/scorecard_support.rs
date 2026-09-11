//! `rule.scorecard` — the detector feedback loop, over a REAL booted `Node`
//! (`docs/scope/insights/case-plane-scope.md` §7, "Feedback from resolution to detection").
//!
//! Real store, real bus, real caps, the real `call_tool` MCP bridge. NO mocks (CLAUDE §9): every
//! insight is raised through `insight.raise`, every case is the one the grouping reactor opened for
//! it, and every outcome is written by `case.workflow`. The scorecard is then read back through the
//! verb, which is the only path a UI has.
//!
//! Mandatory categories: **capability-deny**, **workspace-isolation**, and the **POSITIVE gate
//! test** — a principal holding ONLY `mcp:rule.scorecard:call` must actually reach it, because a
//! cap that exists in no bundle (or a missing `tool_gate.rs` arm) is `Denied`, not `NotFound`, and
//! that refusal is indistinguishable from a real authorization failure
//! (`new-lb-verb-needs-a-gate-alias.md`). This trap has bitten this branch twice.
//!
//! The arithmetic itself has unit tests in `lb_cases::scorecard`; what is proved HERE is the join:
//! that `origin.ref` and `case.resolution` meet correctly after a real round trip through the
//! `{ data, rev }` store envelope.
//!
//! **Fixtures only.** Split out of `scorecard_test.rs` when that file passed the 400-line
//! FILE-LAYOUT limit. The tests live in the `scorecard_*.rs` siblings, aggregated by
//! `case_suite.rs`; this module holds what they share and asserts nothing itself.

#![allow(dead_code)]

pub use std::sync::Arc;

pub use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
pub use lb_authz::membership_add_raw;
pub use lb_host::{call_tool, Node};
pub use lb_mcp::ToolError;
pub use serde_json::{json, Value};

pub const RAISE: &str = "mcp:insight.raise:call";
pub const I_GET: &str = "mcp:insight.get:call";
pub const GET: &str = "mcp:case.get:call";
pub const WORKFLOW: &str = "mcp:case.workflow:call";
pub const SCORECARD: &str = "mcp:rule.scorecard:call";

/// The fully-empowered operator: raise findings, close cases, read the scorecard.
pub const ALL: &[&str] = &[RAISE, I_GET, GET, WORKFLOW, SCORECARD];

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

pub async fn seed_roster(node: &Arc<Node>, ws: &str) {
    membership_add_raw(&node.store, ws, "user:test", 1)
        .await
        .expect("test joins");
}

/// Raise one insight under `origin_ref`, optionally at `site`, and return its id. Grouping runs
/// inline at raise, so this also mints the `single` case the scorecard will later count.
pub async fn seed_insight(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    key: &str,
    origin_ref: &str,
    site: Option<&str>,
    ts: u64,
) -> String {
    let mut input = json!({
        "dedup_key": key,
        "severity": "warning",
        "title": format!("finding {key}"),
        "origin": { "kind": "rule", "ref": origin_ref },
        "ts": ts,
    });
    if let Some(site) = site {
        input["tags"] = json!({ "site": site });
    }
    let out = call(node, p, ws, "insight.raise", input)
        .await
        .expect("raise ok");
    out["id"].as_str().unwrap().to_string()
}

/// The case the grouping reactor opened for `insight_id`, read through the same `case_id` echo a
/// roster renders from.
pub async fn case_of(node: &Arc<Node>, p: &Principal, ws: &str, insight_id: &str) -> String {
    let out = call(node, p, ws, "insight.get", json!({ "id": insight_id }))
        .await
        .expect("get ok");
    out["case_id"]
        .as_str()
        .unwrap_or_else(|| panic!("insight {insight_id} has no case_id echo: {out}"))
        .to_string()
}

/// Raise a finding and close its case with `resolution` at `resolved_ts` — one whole outcome, the
/// unit the scorecard counts. Returns the insight id.
#[allow(clippy::too_many_arguments)]
pub async fn outcome(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    key: &str,
    origin_ref: &str,
    site: Option<&str>,
    opened_ts: u64,
    resolution: &str,
    resolved_ts: u64,
) -> String {
    let insight = seed_insight(node, p, ws, key, origin_ref, site, opened_ts).await;
    let case_id = case_of(node, p, ws, &insight).await;
    call(
        node,
        p,
        ws,
        "case.workflow",
        json!({
            "id": case_id,
            "workflow": "resolved",
            "resolution": resolution,
            "ts": resolved_ts,
        }),
    )
    .await
    .expect("case resolves");
    insight
}

pub async fn scorecard(node: &Arc<Node>, p: &Principal, ws: &str, args: Value) -> Vec<Value> {
    let out = call(node, p, ws, "rule.scorecard", args)
        .await
        .expect("scorecard ok");
    out["rows"].as_array().cloned().unwrap_or_default()
}

/// The one row for `(rule_ref, site)` in a scorecard result.
pub fn row<'a>(rows: &'a [Value], rule_ref: &str, site: Option<&str>) -> &'a Value {
    rows.iter()
        .find(|r| r["rule_ref"] == json!(rule_ref) && r.get("site").and_then(Value::as_str) == site)
        .unwrap_or_else(|| panic!("no row for ({rule_ref}, {site:?}) in {rows:#?}"))
}
