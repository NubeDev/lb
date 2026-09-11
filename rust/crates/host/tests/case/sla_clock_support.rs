//! The **sla-clock reactor** over a REAL booted `Node` (`docs/scope/insights/case-plane-scope.md`
//! §"Reactors", the sla-clock row). Real store, real policies, real business-hours arithmetic, the
//! real reminder reactor, the real `call_tool` MCP bridge. NO mocks.
//!
//! **Every reactor case asserts RED first.** The clock is an inline effect of `insight.raise`, so
//! the capability that gates the path is `mcp:insight.raise:call`; the breach alarm's own path is
//! gated by `mcp:case.breach:call`, which is granted to exactly one subject and nowhere else. Each
//! reactor case therefore runs the identical call with the relevant cap REMOVED, asserts the
//! refusal AND that nothing was written, then restores it and asserts the effect appears. A reactor
//! test that only ever ran green proves nothing (`green-while-broken-reactor-tests.md`): without the
//! RED half, a clock that never ran at all would pass every assertion below that reads a `None`.
//!
//! The calendar throughout is Mon–Fri 09:00–17:00 **UTC**. A fixed-offset zone keeps these cases
//! about the SLA rather than about daylight saving — the DST behaviour belongs to
//! `lb_cases::deadline` and has its own tests there.
//!
//! **Fixtures only.** Split out of `sla_clock_test.rs` when that file passed the 400-line
//! FILE-LAYOUT limit. The tests live in the `sla_clock_*.rs` siblings, aggregated by
//! `case_suite.rs`; this module holds what they share and asserts nothing itself.

#![allow(dead_code)]

pub use std::sync::Arc;

pub use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
pub use lb_authz::{grant_assign, grant_revoke, membership_add_raw, Subject};
pub use lb_host::{call_tool, react_to_reminders, Node};
pub use lb_mcp::ToolError;
pub use serde_json::{json, Value};

pub const RAISE: &str = "mcp:insight.raise:call";
pub const I_GET: &str = "mcp:insight.get:call";
pub const GET: &str = "mcp:case.get:call";
pub const LIST: &str = "mcp:case.list:call";
pub const OPEN: &str = "mcp:case.open:call";
pub const WORKFLOW: &str = "mcp:case.workflow:call";
pub const POLICY_SET: &str = "mcp:policy.sla.set:call";
pub const BREACH: &str = "mcp:case.breach:call";

/// Everything a test author needs. `BREACH` is deliberately NOT in here — see `breach_principal`.
pub const ALL: &[&str] = &[RAISE, I_GET, GET, LIST, OPEN, WORKFLOW, POLICY_SET];

/// The reactor principal's caps MINUS the raise grant — the RED half of the clock's own path.
pub const NO_RAISE: &[&str] = &[I_GET, GET, LIST, OPEN, WORKFLOW, POLICY_SET];

/// The subject the SLA clock signs its writes with and fires its breach alarm under. Mirrored from
/// `host/src/case/sla_clock.rs`; a drift here is caught by `the_alarm_is_armed_under_the_clocks_own_subject`.
pub const SLA_ACTOR: &str = "system:sla-clock";

// ── the calendar and the instants every case below is built from ────────────────────────────────
// 2026-01-09 is a FRIDAY. 2026-01-12 is the Monday after it.
/// Friday 2026-01-09 16:00:00 UTC, in epoch **milliseconds**.
pub const FRIDAY_1600: u64 = 1_767_974_400_000;
/// Friday 2026-01-09 17:00:00 UTC — Friday 16:00 + 1 business hour, i.e. the close of business.
pub const FRIDAY_1700: u64 = 1_767_978_000_000;
/// Monday 2026-01-12 10:00:00 UTC — Friday 16:00 + 2 business hours.
pub const MONDAY_1000: u64 = 1_768_212_000_000;
/// Monday 2026-01-12 16:00:00 UTC — Friday 16:00 + 8 business hours.
pub const MONDAY_1600: u64 = 1_768_233_600_000;
/// Tuesday 2026-01-13 10:00:00 UTC — where `respond_by` lands when the Monday is a holiday.
pub const TUESDAY_1000: u64 = 1_768_298_400_000;

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

/// Mon–Fri 09:00–17:00 UTC, closed at the weekend, with an optional holiday list.
pub fn business_calendar(holidays: &[&str]) -> Value {
    let open = json!({ "open_min": 540, "close_min": 1020 });
    let closed = json!({ "open_min": 0, "close_min": 0 });
    json!({
        "kind": "business",
        "tz": "UTC",
        "hours": [open, open, open, open, open, closed, closed],
        "holidays": holidays,
    })
}

/// Write one policy row. `severity` pins the match axis (`None` ⇒ the workspace default).
///
/// The argument list IS the policy's shape — bundling it into a struct would put a second
/// declaration of `ServicePolicy` in a test file.
#[allow(clippy::too_many_arguments)]
pub async fn set_policy(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    id: &str,
    severity: Option<&str>,
    respond_h: u32,
    resolve_h: u32,
    holidays: &[&str],
) {
    let mut policy = json!({
        "id": id,
        "name": id,
        "respond_h": respond_h,
        "resolve_h": resolve_h,
        "calendar": business_calendar(holidays),
    });
    if let Some(severity) = severity {
        policy["match"] = json!({ "severity": severity });
    }
    call(node, p, ws, "policy.sla.set", policy)
        .await
        .expect("policy.sla.set");
}

pub fn raise_input(dedup_key: &str, severity: &str, ts: u64) -> Value {
    json!({
        "dedup_key": dedup_key,
        "severity": severity,
        "title": format!("finding {dedup_key}"),
        "origin": { "kind": "rule", "ref": "rule:probe" },
        "ts": ts,
    })
}

/// Raise a finding and return the case the grouping reactor put it in, read back through the
/// `case_id` echo the grouping writes onto the insight.
pub async fn raise_and_case(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    dedup_key: &str,
    severity: &str,
    ts: u64,
) -> Value {
    let raised = call(
        node,
        p,
        ws,
        "insight.raise",
        raise_input(dedup_key, severity, ts),
    )
    .await
    .expect("insight.raise");
    let insight_id = raised["id"].as_str().expect("a raised insight").to_string();
    let insight = call(node, p, ws, "insight.get", json!({ "id": insight_id }))
        .await
        .expect("insight.get");
    let case_id = insight["case_id"]
        .as_str()
        .unwrap_or_else(|| panic!("insight {insight_id} has no case_id echo: {insight}"))
        .to_string();
    call(node, p, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("case.get")
}

/// How many open cases exist in the workspace.
pub async fn open_case_count(node: &Arc<Node>, p: &Principal, ws: &str) -> u64 {
    call(node, p, ws, "case.list", json!({ "lane": "watching" }))
        .await
        .expect("case.list")["total"]
        .as_u64()
        .unwrap()
}

/// The workspace roster the case plane's lanes read.
pub async fn seed_roster(node: &Arc<Node>, ws: &str) {
    for sub in ["user:admin", "user:blind", "user:nobody"] {
        membership_add_raw(&node.store, ws, sub, 1)
            .await
            .expect("member joins");
    }
}

pub fn u64_field(case: &Value, key: &str) -> Option<u64> {
    case.get(key).and_then(Value::as_u64)
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// 1. The scope's named case: Friday 16:00 under `respond_h: 2` ⇒ Monday 10:00.
// ═══════════════════════════════════════════════════════════════════════════════════════════════
