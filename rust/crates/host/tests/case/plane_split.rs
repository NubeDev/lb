//! Split — a human case no reactor may re-fold.
//!
//! Part of the `plane` suite (see `plane_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::plane_support::*;

// --- Split --------------------------------------------------------------------------------------

/// `case.split` is the human's escape hatch from a reactor's grouping, and what it produces is
/// permanent: the new case is `grouping: human` and its members are `human_placed`, so no reactor
/// may fold them back.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn split_produces_a_human_case_no_reactor_may_re_fold() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let a = seed_insight(&node, &p, "nube", "split-a", 1).await;
    let b = seed_insight(&node, &p, "nube", "split-b", 2).await;
    let case_a = case_of(&node, &p, "nube", &a).await;
    let case_b = case_of(&node, &p, "nube", &b).await;
    call(
        &node,
        &p,
        "nube",
        "case.merge",
        json!({ "from": case_b, "into": case_a, "ts": 3 }),
    )
    .await
    .expect("merge ok");

    // Splitting EVERY member out is refused — that gesture is a re-title, and it would leave a case
    // citing nothing.
    let err = call(
        &node,
        &p,
        "nube",
        "case.split",
        json!({ "from": case_a, "insight_ids": [a, b], "title": "all of it", "ts": 4 }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");

    let new_case = call(
        &node,
        &p,
        "nube",
        "case.split",
        json!({ "from": case_a, "insight_ids": [b], "title": "a separate fault", "ts": 5 }),
    )
    .await
    .expect("split ok");
    assert_eq!(new_case["grouping"], "human");
    assert_eq!(new_case["primary_insight"], b);

    let members = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": new_case["id"] }),
    )
    .await
    .expect("members ok");
    assert!(
        members["items"][0]["human_placed"] == true,
        "a split member must be human_placed: {members}"
    );
    assert_eq!(case_of(&node, &p, "nube", &b).await, new_case["id"]);
}
