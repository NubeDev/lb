//! The breach alarm under the REAL reminder reactor, armed as the clock's own subject, and workspace-isolated.
//!
//! Part of the `sla_clock` suite (see `sla_clock_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::sla_clock_support::*;

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
