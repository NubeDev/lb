//! The bulk-assign per-item contract, unchanged by the delegation.
//!
//! Part of the `delegation` suite (see `delegation_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::delegation_support::*;

/// The bulk contract is unchanged: per-item results, an explicit over-cap refusal, and a per-item
/// failure for a missing id rather than a failed call.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bulk_assign_keeps_its_per_item_contract() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let a = seed_insight(&node, &p, "nube", "bulk-a", 1).await;
    let b = seed_insight(&node, &p, "nube", "bulk-b", 2).await;

    let out = call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "ids": [a, b, "01NOPE"], "assignee": "user:priya", "ts": 3 }),
    )
    .await
    .expect("bulk assign ok");
    let results = out["results"].as_array().unwrap();
    assert_eq!(results.len(), 3, "{out}");
    assert_eq!(results[0]["ok"], true);
    assert_eq!(results[1]["ok"], true);
    assert_eq!(
        results[2]["ok"], false,
        "a missing id fails per-item: {out}"
    );
    assert!(results[2]["error"]
        .as_str()
        .unwrap()
        .contains("no such insight"));

    // Over the cap the WHOLE call is refused — reported, never silently truncated.
    let too_many: Vec<String> = (0..101).map(|i| format!("id-{i}")).collect();
    let err = call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "ids": too_many, "assignee": "user:priya", "ts": 4 }),
    )
    .await
    .unwrap_err();
    match err {
        ToolError::BadInput(m) => assert!(m.contains("bulk cap"), "{m}"),
        other => panic!("expected BadInput, got {other:?}"),
    }
}
