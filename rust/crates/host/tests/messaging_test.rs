//! The messaging happy path — the S2 exit gate, headless: post a message and (1) a live
//! subscriber sees it appear in real time (MOTION), (2) the durable history has it
//! (STATE), proving "post a message, see it appear" and "history is intact".
//!
//! Each test uses a UNIQUE workspace id: in-process Zenoh peers share a workspace's keyspace
//! by design (debugging/bus/in-process-peers-share-the-keyspace.md), so reusing one id would
//! let concurrent tests' buses collide. A unique id is also the correct semantic — the
//! workspace is the isolation wall (§7).

use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{history, post, subscribe_channel, Node};
use lb_inbox::Item;

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
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
async fn posted_message_appears_to_a_live_subscriber() {
    let ws = "ws-live-subscriber";
    let node = Node::boot().await.expect("node boots");
    let p = principal(ws, &["bus:chan/general:pub", "bus:chan/general:sub"]);

    let sub = subscribe_channel(&node.bus, &p, ws, "general")
        .await
        .expect("subscribe");

    post(
        &node.store,
        &node.bus,
        &p,
        ws,
        "general",
        Item::new("m1", "general", "user:p", "hello world", 1),
    )
    .await
    .expect("post");

    let got = tokio::time::timeout(Duration::from_secs(2), sub.recv())
        .await
        .expect("a message arrives in real time")
        .expect("subscription open");
    assert_eq!(got.id, "m1");
    assert_eq!(got.body, "hello world");
    assert_eq!(got.channel, "general");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn history_survives_independent_of_the_bus() {
    // Post with NO subscriber listening, then read history — the message is there because it
    // was persisted to the store, not because anyone caught the live push. This is the
    // "restart the node and history is intact" guarantee at the store layer.
    let ws = "ws-history-survives";
    let node = Node::boot().await.expect("node boots");
    let p = principal(ws, &["bus:chan/general:pub", "bus:chan/general:sub"]);

    for (i, body) in ["first", "second", "third"].iter().enumerate() {
        post(
            &node.store,
            &node.bus,
            &p,
            ws,
            "general",
            Item::new(format!("m{i}"), "general", "user:p", *body, i as u64),
        )
        .await
        .expect("post");
    }

    let view = history(&node.store, &p, ws, "general")
        .await
        .expect("read history");
    let bodies: Vec<&str> = view.iter().map(|i| i.body.as_str()).collect();
    assert_eq!(
        bodies,
        ["first", "second", "third"],
        "ordered oldest→newest"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_posting_the_same_id_is_idempotent() {
    // Idempotent delivery (inbox-outbox scope): the same (channel, id) upserts one row.
    let ws = "ws-idempotent";
    let node = Node::boot().await.expect("node boots");
    let p = principal(ws, &["bus:chan/general:pub", "bus:chan/general:sub"]);

    for _ in 0..3 {
        post(
            &node.store,
            &node.bus,
            &p,
            ws,
            "general",
            Item::new("dup", "general", "user:p", "once", 1),
        )
        .await
        .expect("post");
    }

    let view = history(&node.store, &p, ws, "general")
        .await
        .expect("read history");
    assert_eq!(view.len(), 1, "re-posting the same id must not duplicate");
}
