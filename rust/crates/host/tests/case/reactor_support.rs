//! The **case-group**, **hold-down** and **reconcile** reactors over a REAL booted `Node`
//! (`docs/scope/insights/case-plane-scope.md` §"Reactors"). Real store, real bus, real caps, the
//! real `call_tool` MCP bridge. NO mocks.
//!
//! **Every grouping case asserts RED first.** Grouping is an inline effect of `insight.raise`, so
//! the capability that gates it is `mcp:insight.raise:call` — the reactor principal's own grant.
//! Each grouping test therefore runs its identical FIRST raise with that cap REMOVED, asserts the
//! refusal AND that no case was created, then runs it with the cap and asserts the case appears. A
//! reactor test that only ever ran green proves nothing (`green-while-broken-reactor-tests.md`):
//! without the RED half a grouping function that returned a hard-coded id, or one that never ran at
//! all, would pass.
//!
//! **The two `reconcile_*` cases are the exception, and deliberately so.** `reconcile_cases` is a
//! node-level backfill invoked as a Rust function, not a verb behind the caps wall, so there is no
//! cap to remove — a "RED half" there would be theatre. Their equivalent is a **pre-pass
//! assertion**: the legacy fixture is written straight through the `lb_insights` writers and the
//! workspace is asserted to hold ZERO cases *before* the pass runs, which is what makes "the pass
//! created it" a claim the test can actually make rather than assume.
//!
//! The two orderings a scheduled rule really produces — **verdict-first** (the citing record beats
//! the findings it cites) and **verdict-last** (the findings already sit in `single` cases) — each
//! get their own case, because they exercise completely different code paths and converge on the
//! same answer only if both are right.
//!
//! **Fixtures only.** Split out of `case_reactor_test.rs` when that file passed the 400-line
//! FILE-LAYOUT limit. The tests live in the `reactor_*.rs` siblings, aggregated by
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
pub const I_COMMENT: &str = "mcp:insight.comment:call";
pub const I_ASSIGN: &str = "mcp:insight.assign:call";
pub const GET: &str = "mcp:case.get:call";
pub const LIST: &str = "mcp:case.list:call";
pub const OPEN: &str = "mcp:case.open:call";
pub const WORKFLOW: &str = "mcp:case.workflow:call";

pub const ALL: &[&str] = &[RAISE, I_GET, I_COMMENT, I_ASSIGN, GET, LIST, OPEN, WORKFLOW];

/// The reactor principal's caps MINUS the raise grant — the RED half of every case below.
pub const NO_RAISE: &[&str] = &[I_GET, I_COMMENT, I_ASSIGN, GET, LIST, OPEN, WORKFLOW];

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

/// A VERDICT record, in the shape the real producer writes: `explains[]` holds the **dedup_key**s of
/// the findings it accounts for, and `root_cause` names the upstream fault it blames.
///
/// The other body keys the producer writes (`equip`, `root_issue`, `first_repair`, `note`) are
/// present deliberately: the grouping must read `root_cause` and `explains` and be completely
/// indifferent to the rest — that indifference IS rule 10, and a test that omitted them would not
/// notice a grouping that had started keying off `equip`.
pub fn verdict_input(dedup_key: &str, root_cause: &str, explains: &[&str], ts: u64) -> Value {
    json!({
        "dedup_key": dedup_key,
        "severity": "critical",
        "title": format!("verdict {dedup_key}"),
        "origin": { "kind": "rule", "ref": "rule:probe" },
        "ts": ts,
        "body": {
            "equip": "some-equip-ref",
            "root_cause": root_cause,
            "root_issue": "some issue text",
            "explains": explains,
            "first_repair": "some repair text",
            "note": "some note",
        },
    })
}

/// A raise straight through the CRATE — the shape a node running before the case plane existed left
/// behind, with no grouping and therefore no case. The backfill's whole reason for existing.
pub fn legacy_raise(
    dedup_key: &str,
    severity: lb_insights::Severity,
    title: &str,
) -> lb_insights::RaiseInput {
    lb_insights::RaiseInput {
        dedup_key: dedup_key.into(),
        severity,
        title: title.into(),
        body: Value::Null,
        evidence: None,
        analysis: None,
        origin: lb_insights::Origin::new(lb_insights::OriginKind::Rule, "rule:legacy", None),
        tags: Default::default(),
        occurrence: None,
        ts: 1_000,
        producer: "key:legacy".into(),
    }
}

pub async fn seed_roster(node: &Arc<Node>, ws: &str) {
    membership_add_raw(&node.store, ws, "user:test", 1)
        .await
        .expect("test joins");
    membership_add_raw(&node.store, ws, "user:priya", 1)
        .await
        .expect("priya joins");
}

/// The case the grouping opened for `insight_id`, via the `case_id` echo.
pub async fn case_of(node: &Arc<Node>, p: &Principal, ws: &str, insight_id: &str) -> String {
    let out = call(node, p, ws, "insight.get", json!({ "id": insight_id }))
        .await
        .expect("get ok");
    out["case_id"]
        .as_str()
        .unwrap_or_else(|| panic!("insight {insight_id} has no case_id echo: {out}"))
        .to_string()
}

/// How many open cases exist in the workspace — the number the invariant is really about.
pub async fn open_case_count(node: &Arc<Node>, p: &Principal, ws: &str) -> u64 {
    call(node, p, ws, "case.list", json!({ "lane": "watching" }))
        .await
        .expect("list ok")["total"]
        .as_u64()
        .unwrap()
}

/// The member ids of a case, sorted.
pub async fn member_ids(node: &Arc<Node>, p: &Principal, ws: &str, case_id: &str) -> Vec<String> {
    let page = call(node, p, ws, "case.members", json!({ "case_id": case_id }))
        .await
        .expect("members ok");
    let mut ids: Vec<String> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["insight_id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    ids
}
