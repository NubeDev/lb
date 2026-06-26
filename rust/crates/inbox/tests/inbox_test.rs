//! The inbox verbs over a real embedded SurrealDB (testing §3): record persists, list reads
//! back ordered, re-recording the same id is idempotent, and the MANDATORY workspace-isolation
//! guarantee holds (testing §2.2) — a list in workspace B never returns workspace A's items.

use lb_inbox::{list, record, Item};
use lb_store::Store;

#[tokio::test]
async fn records_and_lists_in_ts_order() {
    let store = Store::memory().await.expect("open store");
    // Insert out of order; list must return oldest→newest by ts.
    record(&store, "a", &Item::new("m2", "general", "u", "second", 2))
        .await
        .unwrap();
    record(&store, "a", &Item::new("m1", "general", "u", "first", 1))
        .await
        .unwrap();

    let items = list(&store, "a", "general").await.expect("list");
    let bodies: Vec<&str> = items.iter().map(|i| i.body.as_str()).collect();
    assert_eq!(bodies, ["first", "second"]);
}

#[tokio::test]
async fn re_recording_the_same_id_is_idempotent() {
    let store = Store::memory().await.expect("open store");
    for _ in 0..3 {
        record(&store, "a", &Item::new("dup", "general", "u", "once", 1))
            .await
            .unwrap();
    }
    let items = list(&store, "a", "general").await.unwrap();
    assert_eq!(items.len(), 1);
}

#[tokio::test]
async fn channels_are_independent_within_a_workspace() {
    let store = Store::memory().await.expect("open store");
    record(&store, "a", &Item::new("m1", "general", "u", "g", 1))
        .await
        .unwrap();
    record(&store, "a", &Item::new("m1", "random", "u", "r", 1))
        .await
        .unwrap();

    assert_eq!(list(&store, "a", "general").await.unwrap().len(), 1);
    assert_eq!(list(&store, "a", "random").await.unwrap().len(), 1);
}

#[tokio::test]
async fn list_in_one_workspace_never_returns_another_workspaces_items() {
    let store = Store::memory().await.expect("open store");
    record(
        &store,
        "a",
        &Item::new("m1", "general", "u", "secret of A", 1),
    )
    .await
    .unwrap();

    let from_b = list(&store, "b", "general").await.expect("list from b");
    assert!(
        from_b.is_empty(),
        "INBOX LEAK: workspace B saw workspace A's items: {from_b:?}"
    );
}
