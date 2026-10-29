//! The SLA follow pass on the case tick (Insights review 2026-10-29): a case stored before "the clock
//! runs only while Open" — out of Open, no pause on record, its deadline still on the old resolve
//! target — is paused from its last activity and moved onto the respond target, once.
//!
//! Part of the `sla_clock` suite (see `sla_clock_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::sla_clock_support::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_tick_pauses_a_case_stored_before_the_rule_and_moves_its_deadline() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "acme";
    seed_roster(&node, ws).await;
    let admin = principal("user:admin", ws, ALL);
    set_policy(&node, &admin, ws, "default", None, 2, 8, &[]).await;

    let case = raise_and_case(&node, &admin, ws, "k1", "warning", FRIDAY_1600).await;
    let case_id = case["id"].as_str().unwrap().to_string();

    // As it would have been stored before the rule: actioned an hour in, no pause on record, and the
    // deadline on the old 8-hour resolve target.
    let mut old = lb_cases::get(&node.store, ws, &case_id)
        .await
        .unwrap()
        .unwrap();
    old.workflow = lb_cases::Workflow::Actioned;
    old.sla_paused_at = None;
    old.due_at = Some(MONDAY_1600);
    old.last_activity_ts = FRIDAY_1700;
    let value = serde_json::to_value(&old).unwrap();
    lb_store::write(&node.store, ws, "case", &case_id, &value)
        .await
        .unwrap();

    // RED (pre-pass): the record carries the old state.
    let before = call(&node, &admin, ws, "case.get", json!({ "id": case_id }))
        .await
        .unwrap();
    assert!(
        before
            .get("sla_paused_at")
            .map(Value::is_null)
            .unwrap_or(true),
        "{before}"
    );
    assert_eq!(u64_field(&before, "due_at"), Some(MONDAY_1600));

    assert_eq!(
        lb_host::follow_sla_clocks(&node, ws, MONDAY_1600)
            .await
            .unwrap(),
        1
    );
    let after = call(&node, &admin, ws, "case.get", json!({ "id": case_id }))
        .await
        .unwrap();
    assert_eq!(
        u64_field(&after, "sla_paused_at"),
        Some(FRIDAY_1700),
        "paused from its last activity, the best record of when it left Open: {after}"
    );
    assert_eq!(
        u64_field(&after, "due_at"),
        Some(MONDAY_1000),
        "moved onto the two-hour respond target"
    );
    assert_eq!(
        u64_field(&after, "last_activity_ts"),
        Some(FRIDAY_1700),
        "the books are not activity"
    );

    assert_eq!(
        lb_host::follow_sla_clocks(&node, ws, MONDAY_1600 + 60_000)
            .await
            .unwrap(),
        0,
        "idempotent"
    );
}
