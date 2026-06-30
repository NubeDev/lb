//! The message-delete happy path, its live feed, and its denials (channels-edit-delete scope).
//! Like edit, delete is a write gated by `pub` AND by author ownership. Each test uses a UNIQUE
//! workspace id (in-process Zenoh peers share a workspace's keyspace by design).

use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{delete, history, post, watch_deletions, ChannelError, Node};
use lb_inbox::Item;

fn principal(ws: &str, sub: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn author_can_delete_their_own_message() {
    let ws = "ws-del-own";
    let node = Node::boot().await.expect("node boots");
    let alice = principal(
        ws,
        "user:alice",
        &["bus:chan/general:pub", "bus:chan/general:sub"],
    );

    post(
        &node,
        &alice,
        ws,
        "general",
        Item::new("m1", "general", "user:alice", "bye soon", 1),
    )
    .await
    .expect("post");

    delete(&node, &alice, ws, "general", "m1")
        .await
        .expect("delete");

    let view = history(&node.store, &alice, ws, "general")
        .await
        .expect("history");
    assert!(view.is_empty(), "deleted message is gone from history");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_live_viewer_sees_the_deletion() {
    let ws = "ws-del-live";
    let node = Node::boot().await.expect("node boots");
    let alice = principal(
        ws,
        "user:alice",
        &["bus:chan/general:pub", "bus:chan/general:sub"],
    );

    post(
        &node,
        &alice,
        ws,
        "general",
        Item::new("m1", "general", "user:alice", "gone", 1),
    )
    .await
    .expect("post");

    let feed = watch_deletions(&node.bus, &alice, ws, "general")
        .await
        .expect("watch deletions");

    delete(&node, &alice, ws, "general", "m1")
        .await
        .expect("delete");

    let id = tokio::time::timeout(Duration::from_secs(2), feed.recv())
        .await
        .expect("the deletion arrives on the live feed")
        .expect("feed open");
    assert_eq!(id, "m1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_non_author_cannot_delete() {
    let ws = "ws-del-deny-other";
    let node = Node::boot().await.expect("node boots");
    let alice = principal(ws, "user:alice", &["bus:chan/general:pub"]);
    let bob = principal(ws, "user:bob", &["bus:chan/general:pub"]);

    post(
        &node,
        &alice,
        ws,
        "general",
        Item::new("m1", "general", "user:alice", "alice's", 1),
    )
    .await
    .expect("post");

    let err = delete(&node, &bob, ws, "general", "m1")
        .await
        .expect_err("non-author delete is refused");
    assert!(matches!(err, ChannelError::Denied));

    // Still present.
    let viewer = principal(ws, "user:view", &["bus:chan/general:sub"]);
    let view = history(&node.store, &viewer, ws, "general")
        .await
        .expect("history");
    assert_eq!(view.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deleting_a_missing_own_id_is_not_found() {
    let ws = "ws-del-missing";
    let node = Node::boot().await.expect("node boots");
    let alice = principal(ws, "user:alice", &["bus:chan/general:pub"]);

    let err = delete(&node, &alice, ws, "general", "nope")
        .await
        .expect_err("missing id is NotFound");
    assert!(matches!(err, ChannelError::NotFound));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_is_refused_without_a_pub_grant() {
    let ws = "ws-del-deny-cap";
    let node = Node::boot().await.expect("node boots");
    let alice = principal(ws, "user:alice", &["bus:chan/general:pub"]);
    let noluck = principal(ws, "user:noluck", &[]); // no caps

    post(
        &node,
        &alice,
        ws,
        "general",
        Item::new("m1", "general", "user:alice", "hi", 1),
    )
    .await
    .expect("post");

    let err = delete(&node, &noluck, ws, "general", "m1")
        .await
        .expect_err("ungranted delete is refused");
    assert!(matches!(err, ChannelError::Denied));

    // The deny ran before the store was touched.
    let viewer = principal(ws, "user:view", &["bus:chan/general:sub"]);
    let view = history(&node.store, &viewer, ws, "general")
        .await
        .expect("history");
    assert_eq!(view.len(), 1, "a denied delete still erased the store");
}
