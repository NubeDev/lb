//! The two invariants the plane turns on: `resolved` needs a resolution, and an open insight sits
//! in exactly one open case.
//!
//! Part of the `plane` suite (see `plane_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::plane_support::*;

// --- The resolution invariant -------------------------------------------------------------------

/// `case.workflow(resolved)` without a `resolution` is `BadInput`, and the case does not move.
///
/// This is the invariant that keeps a queue from becoming a graveyard: closed-with-no-reason cannot
/// be told from a false positive later, and the hold-down reactor has nothing to judge.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolved_without_a_resolution_is_bad_input_and_nothing_moves() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &p, "nube", "res-probe", 1).await;
    let case_id = case_of(&node, &p, "nube", &insight).await;

    let err = call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "resolved", "ts": 10 }),
    )
    .await
    .unwrap_err();
    match err {
        ToolError::BadInput(m) => assert!(
            m.contains("resolution"),
            "the refusal must name the missing field: {m}"
        ),
        other => panic!("expected BadInput, got {other:?}"),
    }

    let case = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["workflow"], "to_action", "the case did not move");
    assert_eq!(case["closed"], false);

    // The mirror image: a resolution on a NON-terminal transition is refused too, so the two fields
    // can never disagree.
    let err = call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "actioned", "resolution": "fixed", "ts": 11 }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");

    // With a resolution, it closes — and stamps who and when.
    let closed = call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "resolved", "resolution": "fixed", "ts": 12 }),
    )
    .await
    .expect("resolve ok");
    assert_eq!(closed["workflow"], "resolved");
    assert_eq!(closed["resolution"], "fixed");
    assert_eq!(closed["closed"], true);
    assert_eq!(closed["resolved_by"], "user:test");
    assert_eq!(closed["resolved_ts"], 12);
}

// --- The exclusivity invariant ------------------------------------------------------------------

/// **Every open insight is in exactly one open case.** Adding a detection already held by another
/// OPEN case is refused; `case.merge` is the verb that legitimately moves it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_open_insight_is_in_exactly_one_open_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let a = seed_insight(&node, &p, "nube", "excl-a", 1).await;
    let b = seed_insight(&node, &p, "nube", "excl-b", 2).await;
    let case_a = case_of(&node, &p, "nube", &a).await;
    let case_b = case_of(&node, &p, "nube", &b).await;
    assert_ne!(case_a, case_b, "two findings, two `single` cases");

    // Opening a THIRD case that cites `b` — already held by `case_b` — is refused.
    let err = call(
        &node,
        &p,
        "nube",
        "case.open",
        json!({ "title": "a second home for b", "primary_insight": b, "ts": 3 }),
    )
    .await
    .unwrap_err();
    match err {
        ToolError::BadInput(m) => assert!(
            m.contains("already a member of open case"),
            "the refusal must name the invariant: {m}"
        ),
        other => panic!("expected BadInput, got {other:?}"),
    }

    // Merge is the way. After it, `b` sits in `case_a` and `case_b` is closed as `duplicate`.
    call(
        &node,
        &p,
        "nube",
        "case.merge",
        json!({ "from": case_b, "into": case_a, "ts": 4 }),
    )
    .await
    .expect("merge ok");

    let members = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": case_a }),
    )
    .await
    .expect("members ok");
    let ids: Vec<&str> = members["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["insight_id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&a.as_str()) && ids.contains(&b.as_str()),
        "{ids:?}"
    );
    assert_eq!(members["total"], 2);

    let loser = call(&node, &p, "nube", "case.get", json!({ "id": case_b }))
        .await
        .expect("get ok");
    assert_eq!(loser["resolution"], "duplicate");
    assert_eq!(loser["closed"], true);

    // And the echo followed the member: `b` now points at `case_a`.
    assert_eq!(case_of(&node, &p, "nube", &b).await, case_a);
}
