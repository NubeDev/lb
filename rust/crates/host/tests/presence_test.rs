//! Presence via Zenoh liveliness (README §6.2): a member who joins is seen by a watcher, and
//! a member who drops (token released) is seen to leave — no stored "online" flag to go stale.
//! Authorization is the channel `sub` grant; isolation is the workspace wall (presence in one
//! workspace is invisible to another).
//!
//! Unique workspace ids per test — in-process Zenoh peers share a workspace's keyspace
//! (debugging/bus/in-process-peers-share-the-keyspace.md).

use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{join, watch, Node};

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
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
async fn a_joining_member_is_seen_and_leaving_is_seen() {
    let ws = "presence-join-leave";
    let node = Node::boot().await.expect("node boots");
    let p = principal(ws, &["bus:chan/general:sub"]);

    let feed = watch(&node.bus, &p, ws, "general")
        .await
        .expect("watch presence");

    // Member joins (hold the token in a scope so we can drop it to leave).
    {
        let _present = join(&node.bus, &p, ws, "general", "user:alice")
            .await
            .expect("alice joins");

        let (member, present) = tokio::time::timeout(Duration::from_secs(2), feed.recv())
            .await
            .expect("a join event arrives")
            .expect("feed open");
        assert_eq!(member, "user:alice");
        assert!(present, "join event must report present=true");
    } // _present dropped here → alice leaves

    let (member, present) = tokio::time::timeout(Duration::from_secs(2), feed.recv())
        .await
        .expect("a leave event arrives")
        .expect("feed open");
    assert_eq!(member, "user:alice");
    assert!(!present, "drop must report present=false");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn presence_requires_a_sub_grant() {
    // MANDATORY capability-deny on the presence surface: no sub grant → cannot watch or join.
    let ws = "presence-deny";
    let node = Node::boot().await.expect("node boots");
    let p = principal(ws, &[]); // no caps

    assert!(
        watch(&node.bus, &p, ws, "general").await.is_err(),
        "watch without sub must be denied"
    );
    assert!(
        join(&node.bus, &p, ws, "general", "user:p").await.is_err(),
        "join without sub must be denied"
    );
}
