//! The grouping invariant, RED without the reactor's cap and green with it.
//!
//! Part of the `reactor` suite (see `reactor_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::reactor_support::*;

// --- The invariant, RED then green --------------------------------------------------------------

/// **RED first.** With the raise cap removed the grouping never runs: the call is `Denied` and the
/// workspace has zero cases. Only then is the green half meaningful — a `single` case appears with
/// the finding as its primary.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn grouping_is_red_without_the_raise_cap_and_green_with_it() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let reader = principal("node:reactor", "nube", NO_RAISE);

    // RED — the reactor principal LACKS the cap that gates the path.
    let err = call(
        &node,
        &reader,
        "nube",
        "insight.raise",
        raise_input("g1", 1),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "expected Denied: {err:?}");
    assert_eq!(
        open_case_count(&node, &reader, "nube").await,
        0,
        "a denied raise must group nothing"
    );

    // GREEN — the same call, with the cap.
    let full = principal("node:reactor", "nube", ALL);
    let out = call(&node, &full, "nube", "insight.raise", raise_input("g1", 1))
        .await
        .expect("raise ok");
    let id = out["id"].as_str().unwrap();
    assert_eq!(open_case_count(&node, &full, "nube").await, 1);

    let case_id = case_of(&node, &full, "nube", id).await;
    let case = call(&node, &full, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["grouping"], "single");
    assert_eq!(case["primary_insight"], id);
    assert_eq!(case["severity"], "warning", "the severity is echoed");
    assert_eq!(case["closed"], false);
    assert_eq!(member_ids(&node, &full, "nube", &case_id).await, vec![id]);
}
