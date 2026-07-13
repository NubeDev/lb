//! Gap 2 of bus-watch-subject-scope (issue #49): **revoke-terminates-stream**. The subscribe gate
//! runs once, before the stream opens; a `grants.revoke` after that must close an already-open
//! stream within a bounded re-check tick. Proven end to end against a REAL node — `mem://` store,
//! real Zenoh bus, real grants written through the real authz path, the real [`WatchRecheck`] guard
//! the SSE routes use — no mocks (CLAUDE §9, testing §0).
//!
//! We drive [`WatchRecheck::next_authorized`] directly (the exact call the dedicated `GET
//! /bus/{subject}/stream` route folds and the mux hub's `bus:` arm wraps), with a millisecond tick
//! so the test observes the close in real time instead of the production seconds.

mod common;

use std::time::Duration;

use common::*;
use lb_auth::SigningKey;
use lb_host::{bus_watch, Node, Role as NodeRole};
use lb_role_gateway::session::events::WatchRecheck;
use lb_role_gateway::session::verify_token;
use std::sync::Arc;

const WS: &str = "gw-bus-revoke";
const COARSE: &[&str] = &["mcp:bus.watch:call"];
const TICK: Duration = Duration::from_millis(40);

/// Seed `user:<name>` a `bus:<subject>:watch` grant in `ws` through the real grant store.
async fn seed_watch_grant(node: &Node, ws: &str, name: &str, subject: &str) {
    lb_authz::grant_assign(
        &node.store,
        ws,
        &lb_authz::Subject::User(name.into()),
        &format!("bus:{subject}:watch"),
    )
    .await
    .unwrap();
}

async fn revoke_watch_grant(node: &Node, ws: &str, name: &str, subject: &str) {
    lb_authz::grant_revoke(
        &node.store,
        ws,
        &lb_authz::Subject::User(name.into()),
        &format!("bus:{subject}:watch"),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn revoking_the_scoped_grant_closes_the_open_stream_within_a_tick() {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = gateway_on(node.clone(), &key);

    // ada holds the coarse cap + a scoped grant for leo's feed → scoped mode, leo allowed.
    seed_watch_grant(&node, WS, "ada", "care.feed.leo").await;
    let tok = token(&key, "user:ada", WS, COARSE);
    let ada = verify_token(&gw, &tok).await.expect("token verifies");

    // Open the real subscription (the subscribe gate authorizes: scoped grant matches leo).
    let sub = bus_watch(&node.store, &node.bus, &ada, WS, "care.feed.leo")
        .await
        .expect("subscribe authorized");

    let mut recheck = WatchRecheck::with_interval(
        node.store.clone(),
        ada,
        WS.into(),
        "care.feed.leo".into(),
        TICK,
    );

    // Drive the guarded recv in a task. No payloads are published, so it blocks on recv+tick — the
    // tick re-checks the grant. It returns `None` only when the re-check denies (the stream closes).
    let driver = tokio::spawn(async move { recheck.next_authorized(&sub).await });

    // Let a few ticks pass with the grant intact — the stream must STILL be open (not a false close).
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !driver.is_finished(),
        "stream must stay open while the grant is live"
    );

    // Revoke → the next re-check tick must close the stream (return None) within a bounded window.
    revoke_watch_grant(&node, WS, "ada", "care.feed.leo").await;
    let closed = tokio::time::timeout(Duration::from_secs(2), driver)
        .await
        .expect("the stream closes within a bounded tick after revoke")
        .expect("driver task joins");
    assert!(
        closed.is_none(),
        "after revoke the re-check must END the stream (Gap 2)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_open_backcompat_stream_is_not_closed_by_an_unrelated_revoke() {
    // Back-compat: a caller with NO `bus:*:watch` grant is in open mode; the re-check keeps returning
    // authorized, so the stream is never spuriously closed. Revoking some OTHER subject's grant that
    // the caller never held is a no-op for this stream.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = gateway_on(node.clone(), &key);

    let tok = token(&key, "user:ben", WS, COARSE); // ben has no scoped grant → open mode
    let ben = verify_token(&gw, &tok).await.expect("token verifies");
    let sub = bus_watch(&node.store, &node.bus, &ben, WS, "care.feed.leo")
        .await
        .expect("open-mode subscribe authorized");

    let mut recheck = WatchRecheck::with_interval(
        node.store.clone(),
        ben,
        WS.into(),
        "care.feed.leo".into(),
        TICK,
    );
    let driver = tokio::spawn(async move { recheck.next_authorized(&sub).await });

    // Revoke an unrelated grant for a DIFFERENT user; ben's open-mode stream is unaffected.
    revoke_watch_grant(&node, WS, "ada", "care.feed.leo").await;
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(
        !driver.is_finished(),
        "an open-mode (back-compat) stream must not close on an unrelated revoke"
    );
    driver.abort();
}
