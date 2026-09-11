//! The business calendar: a Friday afternoon that responds Monday, a holiday that pushes on, an escalation that recomputes, and a snooze that moves neither deadline.
//!
//! Part of the `sla_clock` suite (see `sla_clock_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::sla_clock_support::*;

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
