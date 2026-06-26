//! The relation edge: idempotent create, list, and revoke-via-tombstone (files scope).

use lb_assets::{list_related, relate, related, unrelate};
use lb_store::Store;

const WS: &str = "ws-relation-test";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn relate_is_idempotent_and_listable() {
    let store = Store::memory().await.unwrap();
    for _ in 0..3 {
        relate(&store, WS, "share", "doc:x", "team:eng")
            .await
            .unwrap();
    }
    relate(&store, WS, "share", "doc:x", "team:design")
        .await
        .unwrap();

    let mut teams = list_related(&store, WS, "share", "doc:x").await.unwrap();
    teams.sort();
    assert_eq!(
        teams,
        vec!["team:design".to_string(), "team:eng".to_string()]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unrelate_revokes_so_related_reads_false() {
    let store = Store::memory().await.unwrap();
    relate(&store, WS, "grant", "skill:s", "ws").await.unwrap();
    assert!(related(&store, WS, "grant", "skill:s", "ws").await.unwrap());

    unrelate(&store, WS, "grant", "skill:s", "ws")
        .await
        .unwrap();
    assert!(!related(&store, WS, "grant", "skill:s", "ws").await.unwrap());
    assert!(list_related(&store, WS, "grant", "skill:s")
        .await
        .unwrap()
        .is_empty());
}
