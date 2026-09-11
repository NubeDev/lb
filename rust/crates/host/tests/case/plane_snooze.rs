//! Snooze and comment semantics, the expired snooze that reads live without a sweep, and the un-forgeable actor on every transition.
//!
//! Part of the `plane` suite (see `plane_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::plane_support::*;

// --- Snooze + comment semantics -----------------------------------------------------------------

/// A snooze needs a REASON; un-snooze is `until: now` and needs none.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_snooze_needs_a_reason_and_un_snooze_is_until_now() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &p, "nube", "snoozer", 1).await;
    let case_id = case_of(&node, &p, "nube", &insight).await;

    let err = call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 9_999_999_999_999u64, "ts": 2 }),
    )
    .await
    .unwrap_err();
    match err {
        ToolError::BadInput(m) => assert!(m.contains("reason"), "{m}"),
        other => panic!("expected BadInput, got {other:?}"),
    }

    let parked = call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 9_999_999_999_999u64, "reason": "tenant moves out in May", "ts": 3 }),
    )
    .await
    .expect("snooze ok");
    assert_eq!(parked["snooze_reason"], "tenant moves out in May");
    assert_eq!(parked["snoozed_by"], "user:test");

    // Un-snooze: `until` at or before now, no reason required.
    let live = call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 10, "ts": 10 }),
    )
    .await
    .expect("un-snooze ok");
    assert!(live["snooze_until"].is_null(), "{live}");
    assert!(live["snooze_reason"].is_null(), "{live}");
}

/// The `snoozed` filter is evaluated against the caller's clock, so an EXPIRED snooze reads as live
/// without any sweep having run.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_expired_snooze_reads_as_live_without_a_sweep() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &p, "nube", "expirer", 1).await;
    let case_id = case_of(&node, &p, "nube", &insight).await;
    call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 5_000, "reason": "a week", "ts": 2 }),
    )
    .await
    .expect("snooze ok");

    let parked = call(
        &node,
        &p,
        "nube",
        "case.list",
        json!({ "lane": "watching", "now": 1_000, "filter": { "snoozed": true } }),
    )
    .await
    .expect("list ok");
    assert_eq!(parked["total"], 1, "parked at now=1000: {parked}");

    let expired = call(
        &node,
        &p,
        "nube",
        "case.list",
        json!({ "lane": "watching", "now": 9_000, "filter": { "snoozed": true } }),
    )
    .await
    .expect("list ok");
    assert_eq!(expired["total"], 0, "live again at now=9000: {expired}");
}

/// The history is append-only and carries every transition — the audit trail a client reads back.
/// `actor` is host-stamped from the principal, so a caller cannot forge another operator's note.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_history_records_every_transition_with_an_un_forgeable_actor() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &p, "nube", "historian", 1).await;
    let case_id = case_of(&node, &p, "nube", &insight).await;

    call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "actioned", "ts": 2 }),
    )
    .await
    .expect("ok");
    call(
        &node,
        &p,
        "nube",
        "case.assign",
        json!({ "id": case_id, "assignee": "user:priya", "ts": 3 }),
    )
    .await
    .expect("ok");
    // A forged author is IGNORED, not refused — the field is simply not read from the input.
    call(
        &node,
        &p,
        "nube",
        "case.comment",
        json!({ "id": case_id, "text": "replaced the sensor", "author": "user:someone-else", "ts": 4 }),
    )
    .await
    .expect("ok");

    let events = call(
        &node,
        &p,
        "nube",
        "case.events",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("events ok");
    let items = events["items"].as_array().unwrap();
    let kinds: Vec<&str> = items.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    for expected in ["opened", "workflow", "assigned", "comment"] {
        assert!(kinds.contains(&expected), "missing `{expected}`: {kinds:?}");
    }
    // Newest first, and every seq unique.
    let seqs: Vec<u64> = items.iter().map(|e| e["eseq"].as_u64().unwrap()).collect();
    let mut sorted = seqs.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(seqs, sorted, "history must be newest-first: {seqs:?}");

    let comment = items.iter().find(|e| e["kind"] == "comment").unwrap();
    assert_eq!(
        comment["actor"], "user:test",
        "the author is host-stamped, never taken from the input: {comment}"
    );
    // The reactor's own writes are visibly `system:`, not a person.
    let opened = items.iter().find(|e| e["kind"] == "opened").unwrap();
    assert_eq!(opened["actor"], "system:case-group");
}
