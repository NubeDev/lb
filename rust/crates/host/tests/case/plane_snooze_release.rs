//! The end of a snooze puts parked work back in the queue: an `actioned` case whose snooze ends — by
//! un-park, by expiry (the case tick's sweep) or by a severity puncture — returns to `to_action`.
//! An `actioned` case somebody holds (`waiting_on`) stays `actioned`, and clearing a snooze that was
//! never set moves nothing.
//!
//! Part of the `plane` suite (see `plane_support.rs` for the fixtures and the full preamble). One
//! binary: `case_suite.rs`.

use super::plane_support::*;

const FAR: u64 = 9_999_999_999_999;

/// One parked case: raised, moved to `stage`, snoozed until `until` with a reason.
async fn parked(node: &Arc<Node>, p: &Principal, key: &str, stage: &str, until: u64) -> String {
    parked_holding(node, p, key, stage, None, until).await
}

/// [`parked`], with who holds the case (`waiting_on`) set on the move.
async fn parked_holding(
    node: &Arc<Node>,
    p: &Principal,
    key: &str,
    stage: &str,
    holder: Option<&str>,
    until: u64,
) -> String {
    let insight = seed_insight(node, p, "nube", key, 1).await;
    let case_id = case_of(node, p, "nube", &insight).await;
    let mut move_to = json!({ "id": case_id, "workflow": stage, "ts": 2 });
    if let Some(h) = holder {
        move_to["waiting_on"] = json!(h);
    }
    call(node, p, "nube", "case.workflow", move_to)
        .await
        .expect("workflow ok");
    call(
        node,
        p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": until, "reason": "Park", "ts": 3 }),
    )
    .await
    .expect("snooze ok");
    case_id
}

async fn get(node: &Arc<Node>, p: &Principal, case_id: &str) -> Value {
    call(node, p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("case.get ok")
}

async fn sweep(node: &Arc<Node>, now: u64) -> usize {
    lb_host::release_expired_snoozes(node, "nube", now)
        .await
        .expect("sweep ok")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn un_parking_an_actioned_case_puts_it_back_to_action() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let case_id = parked(&node, &p, "unpark", "actioned", FAR).await;
    assert_eq!(
        get(&node, &p, &case_id).await["workflow"],
        "actioned",
        "pre: parked in actioned"
    );

    let live = call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 10, "ts": 10 }),
    )
    .await
    .expect("un-snooze ok");
    assert_eq!(live["workflow"], "to_action", "{live}");
    assert!(live["snooze_until"].is_null(), "{live}");

    // A second un-snooze has no snooze to end, so a case someone moved on since is left alone.
    call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "actioned", "ts": 11 }),
    )
    .await
    .expect("workflow ok");
    let again = call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 12, "ts": 12 }),
    )
    .await
    .expect("un-snooze ok");
    assert_eq!(
        again["workflow"], "actioned",
        "never snoozed ⇒ nothing moves: {again}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_expired_snooze_is_released_by_the_sweep_once() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let actioned = parked(&node, &p, "expires", "actioned", 5_000).await;
    let waiting = parked_holding(&node, &p, "waits", "actioned", Some("client"), 5_000).await;
    let later = parked(&node, &p, "later", "actioned", FAR).await;

    assert_eq!(
        sweep(&node, 1_000).await,
        0,
        "nothing has expired at now=1000"
    );
    assert_eq!(get(&node, &p, &actioned).await["workflow"], "actioned");

    assert_eq!(
        sweep(&node, 9_000).await,
        2,
        "both expired snoozes end; the far one does not"
    );
    let back = get(&node, &p, &actioned).await;
    assert_eq!(back["workflow"], "to_action", "{back}");
    assert!(back["snooze_until"].is_null(), "{back}");
    let blocked = get(&node, &p, &waiting).await;
    assert_eq!(
        blocked["workflow"], "actioned",
        "blocked on someone stays: {blocked}"
    );
    assert!(blocked["snooze_until"].is_null(), "{blocked}");
    let still = get(&node, &p, &later).await;
    assert_eq!(still["workflow"], "actioned", "{still}");
    assert!(!still["snooze_until"].is_null(), "{still}");

    // The history says the platform ended it, not a person.
    let events = call(
        &node,
        &p,
        "nube",
        "case.events",
        json!({ "case_id": actioned }),
    )
    .await
    .expect("events ok");
    let items = events["items"].as_array().unwrap();
    let moved = items
        .iter()
        .find(|e| e["kind"] == "workflow" && e["data"]["to"] == "to_action");
    assert_eq!(
        moved.expect("a workflow event back to to_action")["actor"],
        lb_host::SNOOZE_EXPIRY_ACTOR
    );

    assert_eq!(
        sweep(&node, 9_500).await,
        0,
        "idempotent: an ended snooze is not ended twice"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_severity_puncture_puts_a_parked_actioned_case_back_to_action() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let case_id = parked(&node, &p, "worse", "actioned", FAR).await;

    let mut critical = raise_input("worse", 4);
    critical["severity"] = json!("critical");
    call(&node, &p, "nube", "insight.raise", critical)
        .await
        .expect("re-raise ok");

    let case = get(&node, &p, &case_id).await;
    assert_eq!(case["severity"], "critical", "{case}");
    assert!(case["snooze_until"].is_null(), "punctured: {case}");
    assert_eq!(case["workflow"], "to_action", "{case}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn un_parking_a_held_actioned_case_keeps_it_actioned() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    // On hold with a contractor, then snoozed: taking the snooze off must not undo the hand-off.
    let held = parked_holding(&node, &p, "held", "actioned", Some("contractor"), FAR).await;
    let live = call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": held, "until": 10, "ts": 10 }),
    )
    .await
    .expect("un-snooze ok");
    assert_eq!(
        live["workflow"], "actioned",
        "held ⇒ stays actioned: {live}"
    );
    assert_eq!(live["waiting_on"], "contractor", "{live}");
    assert!(live["snooze_until"].is_null(), "{live}");

    // The same on expiry: the sweep ends the snooze and leaves the held case where it is.
    let ours = parked_holding(&node, &p, "ours", "actioned", Some("internal"), 5_000).await;
    assert_eq!(sweep(&node, 9_000).await, 1, "the expired snooze ends");
    let after = get(&node, &p, &ours).await;
    assert_eq!(after["workflow"], "actioned", "{after}");
    assert!(after["snooze_until"].is_null(), "{after}");
}
