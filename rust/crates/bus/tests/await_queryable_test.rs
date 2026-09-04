//! `await_queryable` — the request/reply readiness barrier, tested on two REAL Zenoh peers linked
//! over an explicit loopback endpoint (no mocks, no fixed sleeps in the code under test).
//!
//! What these cover: the barrier releases on a queryable declared **after** the wait began (the
//! shape of the actual race), returns promptly when one is already reachable, and refuses to be
//! satisfied by a declaration in another workspace.
//!
//! **What they do NOT cover, stated plainly:** the implementation declares its `matching_listener`
//! before checking `matching_status`, to close a window in which a queryable appears between the two
//! and its event fires into nothing. Swapping that order was measured against these tests and they
//! still passed — the late-declaration test waits 200 ms, which is far wider than the microsecond
//! gap the ordering protects. That window is not reliably reproducible from a test, so the ordering
//! stands on the argument, not on a guard here. Do not reorder it on the strength of a green run.

use std::time::Duration;

use lb_bus::{await_queryable, declare_queryable, Bus};

/// A loopback port below Linux's default ephemeral range (32768–60999), so the kernel cannot hand
/// the same port to another process between the probe and Zenoh's bind.
fn free_port() -> u16 {
    use std::sync::atomic::{AtomicU16, Ordering};
    static NEXT: AtomicU16 = AtomicU16::new(23_400);
    loop {
        let port = NEXT.fetch_add(1, Ordering::Relaxed);
        assert!(
            port < 32_000,
            "ran out of test ports below the ephemeral range"
        );
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
}

/// Two peers on a deterministic point-to-point link: `server` listens, `client` connects.
async fn linked_pair() -> (Bus, Bus) {
    let endpoint = format!("tcp/127.0.0.1:{}", free_port());
    let server = Bus::peer_with(std::slice::from_ref(&endpoint), &[])
        .await
        .expect("server peer listens");
    let client = Bus::peer_with(&[], &[endpoint])
        .await
        .expect("client peer connects");
    (server, client)
}

/// THE GUARD: the barrier releases on a queryable declared while it is already waiting.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_barrier_sees_a_queryable_declared_after_the_wait_began() {
    let (server, client) = linked_pair().await;

    // Start waiting FIRST, with nothing declared anywhere.
    let waiter =
        tokio::spawn(async move { await_queryable(&client, "ws-a", "some/service").await });

    // Let the waiter reach the barrier, then declare on the other peer. The `_responder` binding
    // matters: dropping it would undeclare the queryable immediately.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let _responder = declare_queryable(&server, "ws-a", "some/service")
        .await
        .expect("server declares its queryable");

    let ready = tokio::time::timeout(Duration::from_secs(15), waiter)
        .await
        .expect(
            "the barrier must release once the queryable is reachable, not hang to its deadline",
        )
        .expect("waiter task")
        .expect("await_queryable");
    assert!(
        ready,
        "a declared, reachable queryable must satisfy the barrier"
    );
}

/// The already-converged case: a queryable declared before the wait satisfies it immediately. This
/// is what the `matching_status` check ahead of the listener loop is for — without it the barrier
/// would wait for a transition event that has already been and gone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_barrier_returns_at_once_when_the_queryable_is_already_there() {
    let (server, client) = linked_pair().await;
    let _responder = declare_queryable(&server, "ws-a", "some/service")
        .await
        .expect("server declares its queryable");

    // Give the declaration time to propagate, so this genuinely exercises the already-matching path
    // rather than the listener path by accident.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let ready = tokio::time::timeout(
        Duration::from_secs(5),
        await_queryable(&client, "ws-a", "some/service"),
    )
    .await
    .expect("an already-reachable queryable must not wait")
    .expect("await_queryable");
    assert!(ready);
}

/// The workspace wall holds on this path too: a queryable declared in workspace A must not satisfy
/// a barrier waiting in workspace B. Without this, the barrier could pass on the wrong node's
/// declaration and hand a routed test a false precondition.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_queryable_in_another_workspace_does_not_satisfy_the_barrier() {
    let (server, client) = linked_pair().await;
    let _responder = declare_queryable(&server, "ws-a", "some/service")
        .await
        .expect("server declares in ws-a");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ws-b has nothing. The barrier's own deadline is long, so bound the wait here instead: a
    // timeout is the PASS, because it means the barrier refused to be satisfied by ws-a's queryable.
    let crossed = tokio::time::timeout(
        Duration::from_secs(2),
        await_queryable(&client, "ws-b", "some/service"),
    )
    .await;
    assert!(
        crossed.is_err(),
        "ws-a's queryable must never satisfy a ws-b barrier — the workspace wall is structural"
    );
}
