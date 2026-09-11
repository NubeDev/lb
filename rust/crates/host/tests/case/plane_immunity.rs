//! Workflow immunity under re-raise — fifty firings leave the human plane untouched — and the severity escalation that punctures a snooze.
//!
//! Part of the `plane` suite (see `plane_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::plane_support::*;

// --- Workflow immunity under re-raise -----------------------------------------------------------

/// **50 re-raises move nothing on the human plane.** A flapping sensor firing every 15 minutes must
/// never resurrect a case somebody parked, un-assign the technician who took the job, or reset a
/// workflow that is waiting on a purchase order. Only the member count and the detection's own
/// lifetime accounting may move.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn fifty_re_raises_leave_the_human_plane_untouched() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &p, "nube", "flapper", 1).await;
    let case_id = case_of(&node, &p, "nube", &insight).await;

    call(
        &node,
        &p,
        "nube",
        "case.assign",
        json!({ "id": case_id, "assignee": "team:mechanical", "ts": 2 }),
    )
    .await
    .expect("assign ok");
    call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "waiting_on_po", "waiting_on": "client", "ts": 3 }),
    )
    .await
    .expect("workflow ok");
    call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 9_999_999_999_999u64, "reason": "PO with the client", "ts": 4 }),
    )
    .await
    .expect("snooze ok");

    let before = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    let members_before = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("members ok");

    // The same dedup key, fifty more times, at the SAME severity (an escalation is the one thing
    // that is allowed to move a snooze, and it is tested separately).
    for i in 0..50u64 {
        call(
            &node,
            &p,
            "nube",
            "insight.raise",
            raise_input("flapper", 100 + i),
        )
        .await
        .expect("re-raise ok");
    }

    let after = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    for field in [
        "workflow",
        "waiting_on",
        "assigned_to",
        "snooze_until",
        "snooze_reason",
        "snoozed_by",
        "resolution",
        "impact_rate",
        "cost_to_fix",
        "verified_saving",
        "reopened_count",
        "id",
    ] {
        assert_eq!(
            before.get(field),
            after.get(field),
            "`{field}` moved under 50 re-raises"
        );
    }

    // And no second case was minted for the same detection.
    let members_after = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("members ok");
    assert_eq!(members_before["total"], members_after["total"]);
    assert_eq!(case_of(&node, &p, "nube", &insight).await, case_id);
}

/// The ONE thing a re-raise may move: a severity **escalation punctures a snooze**. A case parked as
/// "look at it next quarter" that has since become critical is not still parked — leaving it hidden
/// is how a snooze turns into a way to lose a fault.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_severity_escalation_punctures_a_snooze() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &p, "nube", "escalator", 1).await;
    let case_id = case_of(&node, &p, "nube", &insight).await;

    call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 9_999_999_999_999u64, "reason": "next quarter", "ts": 2 }),
    )
    .await
    .expect("snooze ok");
    let parked = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert!(parked["snooze_until"].is_u64(), "parked: {parked}");

    // Same key, worse severity.
    let mut worse = raise_input("escalator", 200);
    worse["severity"] = json!("critical");
    call(&node, &p, "nube", "insight.raise", worse)
        .await
        .expect("re-raise ok");

    let after = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert!(
        after["snooze_until"].is_null(),
        "a severity escalation must puncture the snooze: {after}"
    );
    assert_eq!(after["severity"], "critical", "and carry the new severity");
    // The reason it came back is in the history, not only in the absence of a field.
    let events = call(
        &node,
        &p,
        "nube",
        "case.events",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("events ok");
    let punctured = events["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["kind"] == "snoozed" && e["data"]["punctured"] == true);
    assert!(punctured, "the puncture must be in the history: {events}");
}
