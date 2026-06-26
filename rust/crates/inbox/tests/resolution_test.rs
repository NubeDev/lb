//! The inbox **resolution facet** over a real embedded SurrealDB (testing §3): a decision persists
//! and reads back, re-resolving upserts (last decision wins — a deferred item can later approve),
//! and the MANDATORY workspace-isolation guarantee holds (testing §2.2) — a ws-B read never returns
//! a ws-A resolution. This is the durable record the S6 approval gate reads (coding-workflow scope).

use lb_inbox::{resolution, resolve, Decision, Resolution};
use lb_store::Store;

#[tokio::test]
async fn records_and_reads_a_decision() {
    let store = Store::memory().await.unwrap();
    resolve(
        &store,
        "a",
        &Resolution::new("approve-1", Decision::Approved, "user:ada", 1),
    )
    .await
    .unwrap();

    let r = resolution(&store, "a", "approve-1").await.unwrap().unwrap();
    assert_eq!(r.decision, Decision::Approved);
    assert_eq!(r.actor, "user:ada");
}

#[tokio::test]
async fn re_resolving_upserts_last_decision_wins() {
    // A deferred item can later be approved — the same row is upserted, no duplicate.
    let store = Store::memory().await.unwrap();
    resolve(
        &store,
        "a",
        &Resolution::new("ap", Decision::Deferred, "user:ada", 1),
    )
    .await
    .unwrap();
    resolve(
        &store,
        "a",
        &Resolution::new("ap", Decision::Approved, "user:bob", 2),
    )
    .await
    .unwrap();

    let r = resolution(&store, "a", "ap").await.unwrap().unwrap();
    assert_eq!(r.decision, Decision::Approved, "the later decision wins");
    assert_eq!(r.actor, "user:bob");
}

#[tokio::test]
async fn unresolved_reads_as_none() {
    let store = Store::memory().await.unwrap();
    assert!(resolution(&store, "a", "never").await.unwrap().is_none());
}

#[tokio::test]
async fn a_resolution_is_invisible_across_the_workspace_wall() {
    // MANDATORY workspace-isolation (testing §2.2): a ws-B read can never see a ws-A approval — the
    // namespace is the hard wall (README §7). A leaked approval would defeat the S6 gate.
    let store = Store::memory().await.unwrap();
    resolve(
        &store,
        "iso-a",
        &Resolution::new("ap", Decision::Approved, "user:ada", 1),
    )
    .await
    .unwrap();

    assert!(
        resolution(&store, "iso-b", "ap").await.unwrap().is_none(),
        "ws-B must not see ws-A's approval"
    );
}
