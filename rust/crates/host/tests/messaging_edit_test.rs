//! The message-edit happy path and its denials (channels-edit-delete scope). The edit verb is a
//! write on the channel gated by `pub` AND by author ownership — so the two load-bearing tests
//! are: (1) the author can edit and the change lands in history + propagates live; (2) a
//! non-author is refused, opaquely. Each test uses a UNIQUE workspace id (in-process Zenoh peers
//! share a workspace's keyspace by design).

use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{edit, history, post, subscribe_channel, ChannelError, Node};
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
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn author_can_edit_their_own_message() {
    let ws = "ws-edit-own";
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
        Item::new("m1", "general", "user:alice", "hello", 1),
    )
    .await
    .expect("post");

    let edited = edit(&node, &alice, ws, "general", "m1", "hello (edited)", 2)
        .await
        .expect("edit");
    assert_eq!(edited.body, "hello (edited)");
    assert_eq!(edited.id, "m1");
    assert_eq!(edited.author, "user:alice");

    let view = history(&node.store, &alice, ws, "general")
        .await
        .expect("history");
    assert_eq!(view.len(), 1);
    assert_eq!(view[0].body, "hello (edited)");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_live_subscriber_sees_the_edit() {
    let ws = "ws-edit-live";
    let node = Node::boot().await.expect("node boots");
    let alice = principal(
        ws,
        "user:alice",
        &["bus:chan/general:pub", "bus:chan/general:sub"],
    );

    let sub = subscribe_channel(&node.bus, &alice, ws, "general")
        .await
        .expect("subscribe");

    edit(&node, &alice, ws, "general", "m1", "first", 1)
        .await
        .expect_err("editing a never-posted id is NotFound (ignored here)");

    // Post then edit: the subscriber sees the edit republished on the msg key (id upsert).
    post(
        &node,
        &alice,
        ws,
        "general",
        Item::new("m1", "general", "user:alice", "first", 1),
    )
    .await
    .expect("post");
    let _ = tokio::time::timeout(Duration::from_secs(2), sub.recv())
        .await
        .expect("initial post arrives");

    edit(&node, &alice, ws, "general", "m1", "edited body", 2)
        .await
        .expect("edit");
    let got = tokio::time::timeout(Duration::from_secs(2), sub.recv())
        .await
        .expect("the edit arrives in real time")
        .expect("subscription open");
    assert_eq!(got.id, "m1");
    assert_eq!(got.body, "edited body");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_non_author_cannot_edit() {
    let ws = "ws-edit-deny-other";
    let node = Node::boot().await.expect("node boots");
    let alice = principal(ws, "user:alice", &["bus:chan/general:pub"]);
    let bob = principal(ws, "user:bob", &["bus:chan/general:pub"]);

    post(
        &node,
        &alice,
        ws,
        "general",
        Item::new("m1", "general", "user:alice", "alice's msg", 1),
    )
    .await
    .expect("post");

    let err = edit(&node, &bob, ws, "general", "m1", "hijack", 2)
        .await
        .expect_err("non-author edit is refused");
    assert!(matches!(err, ChannelError::Denied));

    // And the body is untouched.
    let viewer = principal(ws, "user:view", &["bus:chan/general:sub"]);
    let view = history(&node.store, &viewer, ws, "general")
        .await
        .expect("history");
    assert_eq!(view[0].body, "alice's msg");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn editing_a_missing_own_id_is_not_found() {
    let ws = "ws-edit-missing";
    let node = Node::boot().await.expect("node boots");
    let alice = principal(ws, "user:alice", &["bus:chan/general:pub"]);

    let err = edit(&node, &alice, ws, "general", "nope", "x", 1)
        .await
        .expect_err("missing id is NotFound");
    assert!(matches!(err, ChannelError::NotFound));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn edit_is_refused_without_a_pub_grant() {
    let ws = "ws-edit-deny-cap";
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

    let err = edit(&node, &noluck, ws, "general", "m1", "x", 2)
        .await
        .expect_err("ungranted edit is refused");
    assert!(matches!(err, ChannelError::Denied));
}
