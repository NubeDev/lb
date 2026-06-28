//! The first-write (`create`) verb's conditional semantics — the store-layer guarantee the agent's
//! **first-settle Ask decision** leans on (agent-run scope Part 2). Unlike `write` (an upsert),
//! `create` binds on the FIRST write and rejects a second with [`StoreError::Conflict`]. Pure store
//! verbs, no node/bus — a plain `tokio::test`.

use lb_store::{create, read, write, Store, StoreError};
use serde_json::json;

#[tokio::test]
async fn first_create_binds_and_a_second_is_rejected() {
    let store = Store::memory().await.unwrap();
    let ws = "store-create";

    create(
        &store,
        ws,
        "agent_decision",
        "job:c1",
        &json!({"decision": "allow"}),
    )
    .await
    .expect("first create binds");

    // A second create at the same id is rejected — NOT a silent upsert (the first-settle guarantee).
    let second = create(
        &store,
        ws,
        "agent_decision",
        "job:c1",
        &json!({"decision": "deny"}),
    )
    .await;
    assert!(
        matches!(second, Err(StoreError::Conflict)),
        "a second create at the same id is a Conflict, got {second:?}"
    );

    // The bound value is the FIRST one — the second decision did not overwrite it.
    let stored = read(&store, ws, "agent_decision", "job:c1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored, json!({"decision": "allow"}), "first write wins");
}

#[tokio::test]
async fn create_is_walled_per_workspace() {
    // The same id can be created independently in two workspaces — the namespace is the hard wall,
    // and a ws-B create is not blocked by a ws-A record of the same id (README §7).
    let store = Store::memory().await.unwrap();
    create(
        &store,
        "store-create-a",
        "agent_decision",
        "job:c1",
        &json!({"d": "a"}),
    )
    .await
    .unwrap();
    create(
        &store,
        "store-create-b",
        "agent_decision",
        "job:c1",
        &json!({"d": "b"}),
    )
    .await
    .expect("a ws-B create is independent of ws-A");

    let from_a = read(&store, "store-create-a", "agent_decision", "job:c1")
        .await
        .unwrap();
    assert!(from_a.is_none() || from_a == Some(json!({"d": "a"})));
    assert!(
        read(&store, "store-create-a", "agent_decision", "job:c1")
            .await
            .unwrap()
            != Some(json!({"d": "b"})),
        "ws-A must not see ws-B's value"
    );
}

#[tokio::test]
async fn create_conflicts_with_a_prior_plain_write() {
    // A record put there by `write` (upsert) still blocks a later `create` — `create` means "this id
    // must not exist yet", regardless of how it came to exist.
    let store = Store::memory().await.unwrap();
    let ws = "store-create-mix";
    write(
        &store,
        ws,
        "agent_decision",
        "job:c1",
        &json!({"via": "write"}),
    )
    .await
    .unwrap();
    let created = create(
        &store,
        ws,
        "agent_decision",
        "job:c1",
        &json!({"via": "create"}),
    )
    .await;
    assert!(matches!(created, Err(StoreError::Conflict)));
}
