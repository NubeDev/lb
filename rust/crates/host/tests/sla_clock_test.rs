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

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::{grant_assign, grant_revoke, membership_add_raw, Subject};
use lb_host::{call_tool, react_to_reminders, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const RAISE: &str = "mcp:insight.raise:call";
const I_GET: &str = "mcp:insight.get:call";
const GET: &str = "mcp:case.get:call";
const LIST: &str = "mcp:case.list:call";
const OPEN: &str = "mcp:case.open:call";
const WORKFLOW: &str = "mcp:case.workflow:call";
const POLICY_SET: &str = "mcp:policy.sla.set:call";
const BREACH: &str = "mcp:case.breach:call";

/// Everything a test author needs. `BREACH` is deliberately NOT in here — see `breach_principal`.
const ALL: &[&str] = &[RAISE, I_GET, GET, LIST, OPEN, WORKFLOW, POLICY_SET];

/// The reactor principal's caps MINUS the raise grant — the RED half of the clock's own path.
const NO_RAISE: &[&str] = &[I_GET, GET, LIST, OPEN, WORKFLOW, POLICY_SET];

/// The subject the SLA clock signs its writes with and fires its breach alarm under. Mirrored from
/// `host/src/case/sla_clock.rs`; a drift here is caught by `the_alarm_is_armed_under_the_clocks_own_subject`.
const SLA_ACTOR: &str = "system:sla-clock";

// ── the calendar and the instants every case below is built from ────────────────────────────────
// 2026-01-09 is a FRIDAY. 2026-01-12 is the Monday after it.
/// Friday 2026-01-09 16:00:00 UTC, in epoch **milliseconds**.
const FRIDAY_1600: u64 = 1_767_974_400_000;
/// Friday 2026-01-09 17:00:00 UTC — Friday 16:00 + 1 business hour, i.e. the close of business.
const FRIDAY_1700: u64 = 1_767_978_000_000;
/// Monday 2026-01-12 10:00:00 UTC — Friday 16:00 + 2 business hours.
const MONDAY_1000: u64 = 1_768_212_000_000;
/// Monday 2026-01-12 16:00:00 UTC — Friday 16:00 + 8 business hours.
const MONDAY_1600: u64 = 1_768_233_600_000;
/// Tuesday 2026-01-13 10:00:00 UTC — where `respond_by` lands when the Monday is a holiday.
const TUESDAY_1000: u64 = 1_768_298_400_000;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
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

/// Mon–Fri 09:00–17:00 UTC, closed at the weekend, with an optional holiday list.
fn business_calendar(holidays: &[&str]) -> Value {
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
async fn set_policy(
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

fn raise_input(dedup_key: &str, severity: &str, ts: u64) -> Value {
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
async fn raise_and_case(
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
async fn open_case_count(node: &Arc<Node>, p: &Principal, ws: &str) -> u64 {
    call(node, p, ws, "case.list", json!({ "lane": "watching" }))
        .await
        .expect("case.list")["total"]
        .as_u64()
        .unwrap()
}

/// The workspace roster the case plane's lanes read.
async fn seed_roster(node: &Arc<Node>, ws: &str) {
    for sub in ["user:admin", "user:blind", "user:nobody"] {
        membership_add_raw(&node.store, ws, sub, 1)
            .await
            .expect("member joins");
    }
}

fn u64_field(case: &Value, key: &str) -> Option<u64> {
    case.get(key).and_then(Value::as_u64)
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// 1. The scope's named case: Friday 16:00 under `respond_h: 2` ⇒ Monday 10:00.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn friday_afternoon_under_two_business_hours_responds_monday_morning() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    seed_roster(&node, ws).await;
    let admin = principal("user:admin", ws, ALL);
    set_policy(&node, &admin, ws, "default", None, 2, 8, &[]).await;

    // ── RED: the clock rides the raise, so without the raise grant nothing happens at all. ──────
    let blind = principal("user:blind", ws, NO_RAISE);
    let refused = call(
        &node,
        &blind,
        ws,
        "insight.raise",
        raise_input("k1", "warning", FRIDAY_1600),
    )
    .await;
    assert!(
        matches!(refused, Err(ToolError::Denied)),
        "RED half: raise without the cap must be Denied, got {refused:?}"
    );
    assert_eq!(
        open_case_count(&node, &admin, ws).await,
        0,
        "RED half: a refused raise must leave NO case behind for the clock to measure"
    );

    // ── GREEN ───────────────────────────────────────────────────────────────────────────────────
    let case = raise_and_case(&node, &admin, ws, "k1", "warning", FRIDAY_1600).await;
    assert_eq!(case["policy_id"], json!("default"));
    assert_eq!(
        u64_field(&case, "respond_by"),
        Some(MONDAY_1000),
        "Friday 16:00 + 2 business hours spills over the weekend to Monday 10:00"
    );
    assert_eq!(u64_field(&case, "due_at"), Some(MONDAY_1600));
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// 2. The same, across a holiday.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_holiday_pushes_the_deadline_to_the_next_open_day() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    seed_roster(&node, ws).await;
    let admin = principal("user:admin", ws, ALL);
    // The Monday the previous case landed on is closed.
    set_policy(&node, &admin, ws, "default", None, 2, 8, &["2026-01-12"]).await;

    let case = raise_and_case(&node, &admin, ws, "k1", "warning", FRIDAY_1600).await;
    assert_eq!(
        u64_field(&case, "respond_by"),
        Some(TUESDAY_1000),
        "the holiday Monday is skipped entirely; the two hours land on Tuesday"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// 3. A severity escalation recomputes `due_at` AND punctures the snooze.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_escalation_recomputes_the_deadline_and_punctures_the_snooze() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    seed_roster(&node, ws).await;
    let admin = principal("user:admin", ws, ALL);
    set_policy(&node, &admin, ws, "default", None, 2, 8, &[]).await;
    // A tighter clause for the worse thing — the whole point of a severity match axis.
    set_policy(&node, &admin, ws, "urgent", Some("critical"), 1, 1, &[]).await;

    let case = raise_and_case(&node, &admin, ws, "k1", "warning", FRIDAY_1600).await;
    let case_id = case["id"].as_str().unwrap().to_string();
    assert_eq!(case["policy_id"], json!("default"));
    assert_eq!(u64_field(&case, "due_at"), Some(MONDAY_1600));

    // Park it. A snooze is a lane gesture, not a contract amendment.
    call(
        &node,
        &admin,
        ws,
        "case.snooze",
        json!({ "id": case_id, "until": FRIDAY_1600 + 86_400_000 * 30, "reason": "next quarter", "ts": FRIDAY_1600 }),
    )
    .await
    .expect("case.snooze");

    // It got worse.
    let case = raise_and_case(&node, &admin, ws, "k1", "critical", FRIDAY_1600).await;
    assert_eq!(case["severity"], json!("critical"));
    assert_eq!(
        case["policy_id"],
        json!("urgent"),
        "a case that got worse is governed by the clause covering the worse thing"
    );
    assert_eq!(
        u64_field(&case, "due_at"),
        Some(FRIDAY_1700),
        "the tighter clause is one business hour, and Friday 16:00 + 1h is still inside Friday — \
         the escalation pulled the deadline in from Monday 16:00 to the same afternoon"
    );
    assert!(
        case.get("snooze_until").map(Value::is_null).unwrap_or(true),
        "the escalation punctured the snooze: {case}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// 4. A snooze does NOT move either deadline. The clock never pauses.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_snooze_moves_neither_deadline() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    seed_roster(&node, ws).await;
    let admin = principal("user:admin", ws, ALL);
    set_policy(&node, &admin, ws, "default", None, 2, 8, &[]).await;

    let before = raise_and_case(&node, &admin, ws, "k1", "warning", FRIDAY_1600).await;
    let case_id = before["id"].as_str().unwrap().to_string();

    call(
        &node,
        &admin,
        ws,
        "case.snooze",
        json!({ "id": case_id, "until": FRIDAY_1600 + 86_400_000 * 30, "reason": "waiting on the tenant", "ts": FRIDAY_1600 }),
    )
    .await
    .expect("case.snooze");
    // Also put the ball in somebody else's court. `waiting_on` records who holds it; it never stops
    // time, and this is the assertion that says so.
    call(
        &node,
        &admin,
        ws,
        "case.workflow",
        json!({ "id": case_id, "workflow": "waiting_on_po", "waiting_on": "client", "ts": FRIDAY_1600 }),
    )
    .await
    .expect("case.workflow");
    // Re-raise at the SAME severity: the clock runs again and must land on the same instants.
    let after = raise_and_case(&node, &admin, ws, "k1", "warning", FRIDAY_1600 + 60_000).await;

    assert_eq!(
        u64_field(&after, "respond_by"),
        u64_field(&before, "respond_by")
    );
    assert_eq!(u64_field(&after, "due_at"), u64_field(&before, "due_at"));
    assert_eq!(after["policy_id"], before["policy_id"]);
    assert!(
        after["snooze_until"].is_u64(),
        "the same-severity re-raise must NOT have punctured the snooze: {after}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// 5 + 6. At breach: written once, recording the holder at that instant.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_breach_records_the_holder_at_that_instant_and_is_never_rewritten() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    seed_roster(&node, ws).await;
    let admin = principal("user:admin", ws, ALL);
    let clock = principal(SLA_ACTOR, ws, &[BREACH]);
    set_policy(&node, &admin, ws, "default", None, 2, 8, &[]).await;

    let case = raise_and_case(&node, &admin, ws, "k1", "warning", FRIDAY_1600).await;
    let case_id = case["id"].as_str().unwrap().to_string();

    // The contractor has the ball at the moment the deadline passes.
    call(
        &node,
        &admin,
        ws,
        "case.workflow",
        json!({ "id": case_id, "workflow": "actioned", "waiting_on": "contractor", "ts": MONDAY_1000 }),
    )
    .await
    .expect("case.workflow");

    // ── RED: without the cap the alarm's own door refuses, and nothing is written. ──────────────
    let capless = principal(SLA_ACTOR, ws, &[]);
    let refused = call(
        &node,
        &capless,
        ws,
        "case.breach",
        json!({ "case_id": case_id }),
    )
    .await;
    assert!(
        matches!(refused, Err(ToolError::Denied)),
        "RED half: case.breach without the cap must be Denied, got {refused:?}"
    );
    let unbreached = call(&node, &admin, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("case.get");
    assert!(
        unbreached
            .get("breached_ts")
            .map(Value::is_null)
            .unwrap_or(true),
        "RED half: a refused breach must write nothing"
    );

    // ── GREEN ───────────────────────────────────────────────────────────────────────────────────
    let out = call(
        &node,
        &clock,
        ws,
        "case.breach",
        json!({ "case_id": case_id, "ts": MONDAY_1600 + 1 }),
    )
    .await
    .expect("case.breach");
    assert_eq!(out["breached"], json!(true));
    assert_eq!(
        u64_field(&out["case"], "breached_ts"),
        Some(MONDAY_1600),
        "the breach is stamped with the DEADLINE, not the moment the alarm rang"
    );
    assert_eq!(
        out["case"]["breach_waiting_on"],
        json!("contractor"),
        "the holder at that instant is what makes a breach attributable"
    );

    // The ball moves back to us, and somebody runs the alarm again.
    call(
        &node,
        &admin,
        ws,
        "case.workflow",
        json!({ "id": case_id, "workflow": "actioned", "waiting_on": "internal", "ts": MONDAY_1600 + 2 }),
    )
    .await
    .expect("case.workflow");
    let again = call(
        &node,
        &clock,
        ws,
        "case.breach",
        json!({ "case_id": case_id, "ts": MONDAY_1600 + 3 }),
    )
    .await
    .expect("case.breach");
    assert_eq!(
        again["breached"],
        json!(false),
        "a breach is history — a second pass writes nothing"
    );

    let final_case = call(&node, &admin, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("case.get");
    assert_eq!(u64_field(&final_case, "breached_ts"), Some(MONDAY_1600));
    assert_eq!(
        final_case["breach_waiting_on"],
        json!("contractor"),
        "the recorded holder must NOT drift to whoever holds the case today"
    );

    // Exactly one `breach` event, ever.
    let events = call(
        &node,
        &admin,
        ws,
        "case.events",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("case.events");
    let breaches = events["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == json!("breach"))
        .count();
    assert_eq!(breaches, 1, "one breach, one event: {events}");
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// 7. No matching policy ⇒ no deadline. Absent is honest.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn without_a_matching_policy_the_deadlines_stay_absent() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    seed_roster(&node, ws).await;
    let admin = principal("user:admin", ws, ALL);
    // The ONLY policy pins a severity this finding does not carry, so the ladder disqualifies it
    // outright and there is no workspace default to fall back to.
    set_policy(&node, &admin, ws, "urgent", Some("critical"), 1, 1, &[]).await;

    let case = raise_and_case(&node, &admin, ws, "k1", "warning", FRIDAY_1600).await;
    assert!(
        case.get("policy_id").map(Value::is_null).unwrap_or(true),
        "no clause governs this work: {case}"
    );
    assert!(
        case.get("respond_by").map(Value::is_null).unwrap_or(true),
        "an invented default deadline is a promise nobody made: {case}"
    );
    assert!(case.get("due_at").map(Value::is_null).unwrap_or(true));

    // And it SAYS so, rather than leaving a silent blank.
    let events = call(
        &node,
        &admin,
        ws,
        "case.events",
        json!({ "case_id": case["id"] }),
    )
    .await
    .expect("case.events");
    let sla = events["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == json!("sla"))
        .cloned()
        .unwrap_or_else(|| panic!("an sla event must record the absence: {events}"));
    assert_eq!(sla["data"]["unmatched"], json!(true));
    assert_eq!(sla["actor"], json!(SLA_ACTOR));

    // No alarm can be armed for a case that cannot breach a deadline it does not have.
    let id = format!("case-breach:{}", case["id"].as_str().unwrap());
    assert!(
        lb_reminders::load(&node.store, ws, &id)
            .await
            .unwrap()
            .is_none(),
        "no deadline, no alarm"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// 8. The breach alarm ACTUALLY FIRES when the real reminder reactor is driven. RED first.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_breach_alarm_fires_when_the_real_reminder_reactor_is_driven() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    seed_roster(&node, ws).await;
    let admin = principal("user:admin", ws, ALL);
    set_policy(&node, &admin, ws, "default", None, 2, 8, &[]).await;

    let case = raise_and_case(&node, &admin, ws, "k1", "warning", FRIDAY_1600).await;
    let case_id = case["id"].as_str().unwrap().to_string();
    let due_at = u64_field(&case, "due_at").expect("a deadline");

    // The alarm exists, is a ONE-SHOT, and is armed to the exact deadline second.
    let reminder = lb_reminders::load(&node.store, ws, &format!("case-breach:{case_id}"))
        .await
        .expect("store read")
        .expect("the sla-clock armed a breach alarm");
    assert_eq!(reminder.max_runs, Some(1), "a deadline arrives once");
    assert_eq!(reminder.next_attempt_ts, due_at.div_ceil(1000));
    assert_eq!(reminder.principal_sub, SLA_ACTOR);
    assert!(
        lb_reminders::is_valid(&reminder.schedule),
        "the schedule must be a REAL cron — a denied firing reschedules through it: {}",
        reminder.schedule
    );

    // The deadline is in the past relative to the real clock, so the reactor considers it due now.
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(reminder.next_attempt_ts <= now_secs, "the alarm is due");

    // ── RED: revoke the one grant the alarm fires under. The reactor must run, find the alarm due,
    //    and DENY it — writing no breach. This is the half that proves the firing really goes
    //    through the caps wall rather than round it. ──────────────────────────────────────────────
    grant_revoke(
        &node.store,
        ws,
        &Subject::User(SLA_ACTOR.to_string()),
        "mcp:case.breach:call",
    )
    .await
    .expect("revoke");
    let red = react_to_reminders(&node, ws, now_secs)
        .await
        .expect("reactor pass");
    assert_eq!(
        (red.fired, red.denied),
        (0, 1),
        "RED half: the alarm rang and was refused, {red:?}"
    );
    let still_open = call(&node, &admin, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("case.get");
    assert!(
        still_open
            .get("breached_ts")
            .map(Value::is_null)
            .unwrap_or(true),
        "RED half: a denied firing must write no breach"
    );

    // ── GREEN: restore the grant and let the alarm ring. A denied firing already advanced
    //    `next_attempt_ts` past this instant (`react.rs` — leaving it put would wedge the schedule
    //    behind its own idempotency job for ever), so re-arm it one second later: a DIFFERENT
    //    scheduled instant means a different firing job id, which is what lets it fire at all. ────
    grant_assign(
        &node.store,
        ws,
        &Subject::User(SLA_ACTOR.to_string()),
        "mcp:case.breach:call",
    )
    .await
    .expect("grant");
    let mut rearmed = lb_reminders::load(&node.store, ws, &format!("case-breach:{case_id}"))
        .await
        .expect("store read")
        .expect("the alarm survived the denial");
    rearmed.next_attempt_ts = now_secs;
    lb_reminders::save(&node.store, ws, &rearmed)
        .await
        .expect("re-arm");

    let green = react_to_reminders(&node, ws, now_secs)
        .await
        .expect("reactor pass");
    assert_eq!(
        (green.fired, green.denied),
        (1, 0),
        "GREEN half: the alarm fired, {green:?}"
    );

    let breached = call(&node, &admin, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("case.get");
    assert_eq!(
        u64_field(&breached, "breached_ts"),
        Some(due_at),
        "the alarm's firing wrote the breach onto the case: {breached}"
    );

    // And it is spent: one firing, ever.
    let spent = lb_reminders::load(&node.store, ws, &format!("case-breach:{case_id}"))
        .await
        .expect("store read")
        .expect("the alarm record is kept for audit");
    assert_eq!(spent.runs, 1);
    assert_eq!(spent.status, lb_reminders::ReminderStatus::Done);
    assert!(!spent.enabled);
}

/// The alarm holds exactly one capability, under its own subject, in its own workspace — and that
/// capability is in NO role bundle, so this grant is the only thing in the workspace that has it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_alarm_is_armed_under_the_clocks_own_subject() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    seed_roster(&node, ws).await;
    let admin = principal("user:admin", ws, ALL);
    set_policy(&node, &admin, ws, "default", None, 2, 8, &[]).await;
    raise_and_case(&node, &admin, ws, "k1", "warning", FRIDAY_1600).await;

    let granted = lb_authz::granted(
        &node.store,
        ws,
        &Subject::User(SLA_ACTOR.to_string()),
        "mcp:case.breach:call",
    )
    .await
    .expect("grant read");
    assert!(granted, "the clock must be able to fire its own alarm");

    // A brand-new member of the workspace does NOT get it by being a member.
    let member = principal("user:nobody", ws, ALL);
    let refused = call(
        &node,
        &member,
        ws,
        "case.breach",
        json!({ "case_id": "anything" }),
    )
    .await;
    assert!(
        matches!(refused, Err(ToolError::Denied)),
        "a person does not declare a breach: {refused:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// 9. Workspace isolation on the one verb this slice exposes.
// ═══════════════════════════════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn case_breach_is_workspace_isolated() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "acme").await;
    let admin_a = principal("user:admin", "acme", ALL);
    set_policy(&node, &admin_a, "acme", "default", None, 2, 8, &[]).await;
    let case = raise_and_case(&node, &admin_a, "acme", "k1", "warning", FRIDAY_1600).await;
    let case_id = case["id"].as_str().unwrap().to_string();

    // A ws-B token holding the cap FOR ws-B cannot reach into ws-A. The workspace check runs first,
    // so this is refused before the capability is even consulted.
    let clock_b = principal(SLA_ACTOR, "beta", &[BREACH]);
    let refused = call(
        &node,
        &clock_b,
        "acme",
        "case.breach",
        json!({ "case_id": case_id }),
    )
    .await;
    assert!(
        matches!(refused, Err(ToolError::Denied)),
        "cross-workspace breach must be Denied, got {refused:?}"
    );

    // The same id in ws-B names nothing, so even a legitimate ws-B call breaches nothing there.
    let out = call(
        &node,
        &clock_b,
        "beta",
        "case.breach",
        json!({ "case_id": case_id }),
    )
    .await;
    assert!(
        matches!(out, Err(ToolError::BadInput(_))),
        "a ws-B clock finds no such case: {out:?}"
    );

    let untouched = call(
        &node,
        &admin_a,
        "acme",
        "case.get",
        json!({ "id": case_id }),
    )
    .await
    .expect("case.get");
    assert!(untouched
        .get("breached_ts")
        .map(Value::is_null)
        .unwrap_or(true));
}
