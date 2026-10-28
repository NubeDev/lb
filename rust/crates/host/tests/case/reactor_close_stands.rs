//! A person's close holds until the alarm fires again: the reconcile backstop no longer reopens (or
//! re-opens a new case for) an insight whose case was closed after its last firing, while a new
//! firing still reopens a recent fix and opens a new case after a false positive.
//!
//! Part of the `reactor` suite (see `reactor_support.rs` for the fixtures). One binary:
//! `case_suite.rs`.

use super::reactor_support::*;

const DAY: u64 = 24 * 60 * 60 * 1000;

async fn close(node: &Arc<Node>, p: &Principal, case_id: &str, resolution: &str, ts: u64) {
    call(
        node,
        p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "resolved", "resolution": resolution, "reason": "closed on an active alarm", "ts": ts }),
    )
    .await
    .expect("close ok");
}

/// Closed as `fixed` while the alarm is still active: the backstop leaves it closed; a new firing
/// reopens the same case ("repair did not hold").
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_fix_on_an_active_alarm_holds_until_it_fires_again() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let id = call(&node, &p, "nube", "insight.raise", raise_input("cs-1", DAY))
        .await
        .expect("raise ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let case_id = case_of(&node, &p, "nube", &id).await;
    close(&node, &p, &case_id, "fixed", 2 * DAY).await;

    // The insight is still open (the alarm is active); the backstop must not undo the close.
    let grouped = lb_host::reconcile_cases(&node, "nube", 2 * DAY + 60_000)
        .await
        .expect("reconcile ok");
    assert_eq!(grouped, 0, "nothing regrouped: the close stands");
    assert_eq!(open_case_count(&node, &p, "nube").await, 0);
    let kase = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(kase["closed"], true);
    assert_eq!(kase["resolution_reason"], "closed on an active alarm");

    // It fires again: the repair did not hold, the same case comes back.
    call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("cs-1", 3 * DAY),
    )
    .await
    .expect("re-raise ok");
    assert_eq!(case_of(&node, &p, "nube", &id).await, case_id);
    let reopened = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(reopened["closed"], false);
    assert_eq!(reopened["reopened_count"], 1);
}

/// Closed as a false positive while active: the backstop leaves it closed; a new firing opens a
/// NEW case, as the hold-down rules say.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_false_positive_on_an_active_alarm_holds_until_it_fires_again() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let id = call(&node, &p, "nube", "insight.raise", raise_input("cs-2", DAY))
        .await
        .expect("raise ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let first = case_of(&node, &p, "nube", &id).await;
    close(&node, &p, &first, "false_positive", 2 * DAY).await;

    let grouped = lb_host::reconcile_cases(&node, "nube", 2 * DAY + 60_000)
        .await
        .expect("reconcile ok");
    assert_eq!(grouped, 0);
    assert_eq!(
        open_case_count(&node, &p, "nube").await,
        0,
        "no new case for an alarm that did not fire again"
    );

    call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("cs-2", 3 * DAY),
    )
    .await
    .expect("re-raise ok");
    let second = case_of(&node, &p, "nube", &id).await;
    assert_ne!(
        second, first,
        "a false positive is not held down: a new case"
    );
    assert_eq!(open_case_count(&node, &p, "nube").await, 1);
}

/// The backstop still does its job: an open insight that never had a case gets one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_insight_closed_before_its_last_firing_is_still_regrouped() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let id = call(&node, &p, "nube", "insight.raise", raise_input("cs-3", DAY))
        .await
        .expect("raise ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let first = case_of(&node, &p, "nube", &id).await;
    // Closed BEFORE the insight's last firing (a close stamped earlier than `last_ts`): that close
    // does not cover the latest firing, so the backstop gives the insight a case again.
    close(&node, &p, &first, "false_positive", DAY / 2).await;
    let grouped = lb_host::reconcile_cases(&node, "nube", 2 * DAY)
        .await
        .expect("reconcile ok");
    assert_eq!(grouped, 1);
    assert_eq!(open_case_count(&node, &p, "nube").await, 1);
    let _ = id;
}
