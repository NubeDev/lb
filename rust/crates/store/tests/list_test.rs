//! The `list` verb: filter rows by a `data` field within a workspace namespace, with the
//! MANDATORY workspace-isolation guarantee (testing §2.2) — a list in workspace B never
//! returns workspace A's rows, even at the same table/field/value. Real embedded SurrealDB
//! (testing §3), not a mock.

use lb_store::{list, write, Store};
use serde_json::json;

#[tokio::test]
async fn lists_only_rows_matching_the_field_value() {
    let store = Store::memory().await.expect("open store");
    write(
        &store,
        "a",
        "inbox",
        "1",
        &json!({ "channel": "general", "ts": 1 }),
    )
    .await
    .unwrap();
    write(
        &store,
        "a",
        "inbox",
        "2",
        &json!({ "channel": "general", "ts": 2 }),
    )
    .await
    .unwrap();
    write(
        &store,
        "a",
        "inbox",
        "3",
        &json!({ "channel": "random", "ts": 3 }),
    )
    .await
    .unwrap();

    let general = list(&store, "a", "inbox", "channel", "general")
        .await
        .expect("list general");
    assert_eq!(general.len(), 2, "only the two general rows match");
    assert!(general.iter().all(|v| v["channel"] == "general"));
}

#[tokio::test]
async fn list_in_one_workspace_never_returns_another_workspaces_rows() {
    let store = Store::memory().await.expect("open store");
    write(
        &store,
        "a",
        "inbox",
        "1",
        &json!({ "channel": "general", "ts": 1 }),
    )
    .await
    .unwrap();

    // Same table, same field/value, different workspace → empty (namespace wall).
    let from_b = list(&store, "b", "inbox", "channel", "general")
        .await
        .expect("list from b");
    assert!(
        from_b.is_empty(),
        "STORE LEAK: workspace B's list returned workspace A's rows: {from_b:?}"
    );
}

#[tokio::test]
async fn rejects_a_non_identifier_field_name() {
    // The field is interpolated into the query, so a non-identifier must be refused (the guard
    // that makes query injection through `field` impossible).
    let store = Store::memory().await.expect("open store");
    let bad = list(&store, "a", "inbox", "channel = 'x' OR 1=1; --", "general").await;
    assert!(bad.is_err(), "a non-identifier field must be rejected");
}
