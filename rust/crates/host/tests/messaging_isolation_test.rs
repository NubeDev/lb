//! MANDATORY workspace-isolation (testing §2.2) for the messaging slice — across ALL three
//! surfaces the channel touches: the BUS (a live subscriber), the STORE (durable history),
//! and the INBOX (the normalized item). A subscriber in workspace B must NEVER receive — by
//! push or by read — a message posted in workspace A, even holding the matching capability.
//!
//! This is the structural guarantee of §7 proven end to end: the workspace wall is gate 1 of
//! `caps::check` (refusing cross-workspace before any capability) AND the `ws/{id}/` bus
//! prefix + the namespace-per-workspace store (so even an authorized B-call names B's keys).
//!
//! Any test that boots a Node boots a Zenoh peer → it MUST use the multi-thread flavor
//! (debugging/bus/zenoh-needs-multi-thread-runtime.md).

use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{history, post, subscribe_channel, Node};
use lb_inbox::Item;

/// Factory: a verified principal in `ws` holding `caps` (testing §3 fixtures).
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
async fn subscriber_in_ws_b_never_receives_a_publish_in_ws_a() {
    let node = Node::boot().await.expect("node boots");

    // Two distinct workspaces. A may post in WS_A; B holds the SAME capability shape but is
    // scoped to WS_B. The ids are unique to this test so other test binaries can't collide.
    let ws_a = "iso-bus-a";
    let ws_b = "iso-bus-b";
    let a = principal(ws_a, &["bus:chan/general:pub"]);
    let b = principal(ws_b, &["bus:chan/general:sub"]);

    // B subscribes to its own workspace's channel. Its subscription key is `ws/iso-bus-b/...`
    // — it physically cannot name `ws/iso-bus-a/...`.
    let b_sub = subscribe_channel(&node.bus, &b, ws_b, "general")
        .await
        .expect("B subscribes in its own workspace");

    // A posts in WS_A.
    post(
        &node,
        &a,
        ws_a,
        "general",
        Item::new("m1", "general", "user:a", "secret for ws_a only", 1),
    )
    .await
    .expect("A posts in acme");

    // B must receive NOTHING — wait a beat, then assert no message arrived.
    let leaked = tokio::time::timeout(Duration::from_millis(300), b_sub.recv()).await;
    assert!(
        leaked.is_err(),
        "BUS LEAK: workspace B received a message published in workspace A"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn history_in_ws_b_never_returns_ws_a_items() {
    // STORE + INBOX surface: the durable read must be workspace-walled too. B, fully
    // authorized in its OWN workspace, reads an empty channel — A's items are in A's
    // namespace and invisible.
    let ws_a = "iso-store-a";
    let ws_b = "iso-store-b";
    let node = Node::boot().await.expect("node boots");
    let a = principal(ws_a, &["bus:chan/general:pub", "bus:chan/general:sub"]);
    let b = principal(ws_b, &["bus:chan/general:sub"]);

    post(
        &node,
        &a,
        ws_a,
        "general",
        Item::new("m1", "general", "user:a", "ws_a only", 1),
    )
    .await
    .expect("A posts in ws_a");

    let b_view = history(&node.store, &b, ws_b, "general")
        .await
        .expect("B reads its own (empty) channel");
    assert!(
        b_view.is_empty(),
        "STORE LEAK: workspace B's history returned workspace A's items: {b_view:?}"
    );

    // And A's own history does contain it — proving the message really was stored (the empty
    // B result is isolation, not a write that silently failed).
    let a_view = history(&node.store, &a, ws_a, "general")
        .await
        .expect("A reads ws_a");
    assert_eq!(a_view.len(), 1);
    assert_eq!(a_view[0].body, "ws_a only");
}
