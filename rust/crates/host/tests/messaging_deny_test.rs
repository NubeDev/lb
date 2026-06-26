//! MANDATORY capability-deny (testing §2.1) for the messaging slice. Capability-first means
//! the important test is the NEGATIVE one: without the grant, the channel verb is refused —
//! before any bus or store access. One deny per surface verb: post needs `pub`, read/listen
//! need `sub`.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{history, post, subscribe_channel, ChannelError, Node};
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
async fn post_is_refused_without_a_pub_grant() {
    let ws = "ws-deny-post";
    let node = Node::boot().await.expect("node boots");
    let p = principal(ws, &[]); // no caps

    let err = post(
        &node.store,
        &node.bus,
        &p,
        ws,
        "general",
        Item::new("m1", "general", "user:p", "hi", 1),
    )
    .await
    .expect_err("ungranted post is refused");
    assert!(matches!(err, ChannelError::Denied));

    // And nothing leaked into the store — the deny ran before any write.
    let admin = principal(ws, &["bus:chan/general:sub"]);
    let view = history(&node.store, &admin, ws, "general")
        .await
        .expect("read empty channel");
    assert!(view.is_empty(), "a denied post still wrote to the store");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn read_is_refused_without_a_sub_grant() {
    let ws = "ws-deny-read";
    let node = Node::boot().await.expect("node boots");
    // Holds pub (can post) but NOT sub (cannot read/listen).
    let p = principal(ws, &["bus:chan/general:pub"]);

    let read_err = history(&node.store, &p, ws, "general")
        .await
        .expect_err("read without sub is refused");
    assert!(matches!(read_err, ChannelError::Denied));

    let sub_result = subscribe_channel(&node.bus, &p, ws, "general").await;
    assert!(
        matches!(sub_result, Err(ChannelError::Denied)),
        "subscribe without sub must be denied"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_grant_on_a_different_channel_does_not_grant_this_one() {
    // The capability is channel-specific: a grant on `chan/random` must not authorize
    // `chan/general` (the grammar's single-segment `*` does not span here).
    let ws = "ws-deny-other-channel";
    let node = Node::boot().await.expect("node boots");
    let p = principal(ws, &["bus:chan/random:pub"]);

    let err = post(
        &node.store,
        &node.bus,
        &p,
        ws,
        "general",
        Item::new("m1", "general", "user:p", "hi", 1),
    )
    .await
    .expect_err("grant on another channel does not authorize this one");
    assert!(matches!(err, ChannelError::Denied));
}
