//! The hold-down window: a re-fire after a fix reopens, after a false positive opens anew, and long after a fix opens anew.
//!
//! Part of the `reactor` suite (see `reactor_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::reactor_support::*;

// --- Hold-down ----------------------------------------------------------------------------------

/// **`fixed` then a re-fire inside the window ⇒ the SAME case reopens**, `reopened_count: 1`, with a
/// `reopened` event saying the repair did not hold.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_re_fire_after_a_fix_reopens_the_same_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;

    // RED.
    let denied = principal("node:reactor", "nube", NO_RAISE);
    assert!(matches!(
        call(
            &node,
            &denied,
            "nube",
            "insight.raise",
            raise_input("hd-1", 1)
        )
        .await
        .unwrap_err(),
        ToolError::Denied
    ));
    assert_eq!(open_case_count(&node, &denied, "nube").await, 0);

    // GREEN.
    let p = principal("user:test", "nube", ALL);
    let day: u64 = 24 * 60 * 60 * 1000;
    let id = call(&node, &p, "nube", "insight.raise", raise_input("hd-1", day))
        .await
        .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let case_id = case_of(&node, &p, "nube", &id).await;

    call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "resolved", "resolution": "fixed", "ts": 2 * day }),
    )
    .await
    .expect("resolve ok");
    assert_eq!(
        open_case_count(&node, &p, "nube").await,
        0,
        "the queue is clear"
    );

    // Three days later — well inside the default 14-day hold-down — the fault is back.
    call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("hd-1", 5 * day),
    )
    .await
    .expect("re-raise ok");

    assert_eq!(
        case_of(&node, &p, "nube", &id).await,
        case_id,
        "the SAME case must come back, not a new one"
    );
    let reopened = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(reopened["workflow"], "to_action");
    assert_eq!(reopened["closed"], false);
    assert!(
        reopened["resolution"].is_null(),
        "a reopened case must not carry the last repair's closure reason: {reopened}"
    );
    assert!(reopened["resolved_ts"].is_null());
    assert_eq!(reopened["reopened_count"], 1);
    assert_eq!(open_case_count(&node, &p, "nube").await, 1);

    // The history says WHY it came back.
    let events = call(
        &node,
        &p,
        "nube",
        "case.events",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("events ok");
    let reopen_event = events["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "reopened")
        .unwrap_or_else(|| panic!("no `reopened` event: {events}"));
    assert_eq!(reopen_event["data"]["reason"], "repair did not hold");
}

/// **Any other resolution ⇒ a NEW case.** `false_positive` says the detection was wrong; a re-fire is
/// evidence about the rule, not about a repair that failed, so reopening would be a lie about what
/// happened.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_re_fire_after_a_false_positive_opens_a_new_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let day: u64 = 24 * 60 * 60 * 1000;

    // RED — the same first raise with the reactor's grant removed: refused, and nothing grouped.
    // Without this half the assertions below would still pass if grouping never ran at all.
    let no_raise = principal("node:reactor", "nube", NO_RAISE);
    let err = call(
        &node,
        &no_raise,
        "nube",
        "insight.raise",
        raise_input("hd-2", day),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "expected Denied: {err:?}");
    assert_eq!(
        open_case_count(&node, &no_raise, "nube").await,
        0,
        "a denied raise must group nothing"
    );

    let id = call(&node, &p, "nube", "insight.raise", raise_input("hd-2", day))
        .await
        .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let first = case_of(&node, &p, "nube", &id).await;
    call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": first, "workflow": "resolved", "resolution": "false_positive", "ts": 2 * day }),
    )
    .await
    .expect("resolve ok");

    call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("hd-2", 3 * day),
    )
    .await
    .expect("re-raise ok");

    let second = case_of(&node, &p, "nube", &id).await;
    assert_ne!(
        second, first,
        "a false positive must not reopen — a NEW case"
    );
    let old = call(&node, &p, "nube", "case.get", json!({ "id": first }))
        .await
        .expect("get ok");
    assert_eq!(old["closed"], true, "and the old case stays closed");
    assert_eq!(old["resolution"], "false_positive");
}

/// **Outside the window ⇒ a new case**, even after a `fixed`. Nine months later the same fault is
/// next year's job, not last year's unfinished one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_re_fire_long_after_a_fix_opens_a_new_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let day: u64 = 24 * 60 * 60 * 1000;

    // RED — the same first raise with the reactor's grant removed: refused, and nothing grouped.
    // Without this half the assertions below would still pass if grouping never ran at all.
    let no_raise = principal("node:reactor", "nube", NO_RAISE);
    let err = call(
        &node,
        &no_raise,
        "nube",
        "insight.raise",
        raise_input("hd-3", day),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "expected Denied: {err:?}");
    assert_eq!(
        open_case_count(&node, &no_raise, "nube").await,
        0,
        "a denied raise must group nothing"
    );

    let id = call(&node, &p, "nube", "insight.raise", raise_input("hd-3", day))
        .await
        .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let first = case_of(&node, &p, "nube", &id).await;
    call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": first, "workflow": "resolved", "resolution": "fixed", "ts": 2 * day }),
    )
    .await
    .expect("resolve ok");

    // 300 days later — far outside any sane hold-down.
    call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("hd-3", 300 * day),
    )
    .await
    .expect("re-raise ok");

    assert_ne!(
        case_of(&node, &p, "nube", &id).await,
        first,
        "outside the window the repair DID hold; this is new work"
    );
}
