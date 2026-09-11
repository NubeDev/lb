//! The breach record — who held it at that instant, never rewritten — and the absent deadlines when no policy matches.
//!
//! Part of the `sla_clock` suite (see `sla_clock_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::sla_clock_support::*;

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
