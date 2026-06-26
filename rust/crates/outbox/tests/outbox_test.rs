//! The store-layer guarantees the must-deliver outbox leans on (outbox scope, testing §2): the
//! transactional enqueue is atomic, the relay's pending scan is a durable backstop, re-delivery is
//! idempotent, and the workspace wall holds. Pure store verbs — no node/bus — so a plain
//! `tokio::test` (no Zenoh peer) is enough.

use lb_outbox::{enqueue, mark_delivered, mark_failed, pending, Effect, EffectStatus};
use lb_store::{read, Store};
use serde_json::json;

/// A domain change + an effect, enqueued together. Returns the effect id.
async fn enqueue_pr(store: &Store, ws: &str, eff_id: &str, key: &str, ts: u64) {
    let change = json!({ "kind": "job-step", "note": "drafted PR" });
    let effect = Effect::new(
        eff_id,
        "github",
        "create_pr",
        r#"{"repo":"acme/api","head":"fix/2451"}"#,
        key,
        ts,
    );
    enqueue(store, ws, "job", "sess-1", &change, &effect)
        .await
        .unwrap();
}

#[tokio::test]
async fn enqueue_writes_the_change_and_the_effect_atomically() {
    // The transactional-outbox pattern (outbox scope): both records land together. After enqueue,
    // BOTH the domain change and the pending effect are durable.
    let store = Store::memory().await.unwrap();
    let ws = "outbox-tx";
    enqueue_pr(&store, ws, "e1", "pr:2451", 1).await;

    let change = read(&store, ws, "job", "sess-1").await.unwrap();
    assert!(change.is_some(), "the domain change committed");

    let effs = pending(&store, ws).await.unwrap();
    assert_eq!(
        effs.len(),
        1,
        "the effect committed in the same transaction"
    );
    assert_eq!(effs[0].status, EffectStatus::Pending);
    assert_eq!(effs[0].idempotency_key, "pr:2451");
}

#[tokio::test]
async fn a_failed_delivery_stays_schedulable_and_redelivers() {
    // The at-least-once retry (outbox scope offline/sync): a target that fails the first attempt
    // leaves the effect schedulable; the next pending scan still returns it, and on success it ends
    // `delivered` — never lost.
    let store = Store::memory().await.unwrap();
    let ws = "outbox-retry";
    enqueue_pr(&store, ws, "e1", "pr:2451", 1).await;

    // Pass 1: the target is down → mark failed. The effect is still owed.
    let p1 = pending(&store, ws).await.unwrap();
    assert_eq!(p1.len(), 1);
    mark_failed(&store, ws, "e1").await.unwrap();

    // Pass 2: it is STILL pending-for-the-relay (failed is schedulable), now with attempts counted.
    let p2 = pending(&store, ws).await.unwrap();
    assert_eq!(p2.len(), 1, "a failed effect re-appears for re-delivery");
    assert_eq!(p2[0].status, EffectStatus::Failed);
    assert_eq!(p2[0].attempts, 1, "the failed attempt was counted");

    // Pass 2 succeeds → delivered. It drops out of the schedulable set (no double-send).
    mark_delivered(&store, ws, "e1").await.unwrap();
    let p3 = pending(&store, ws).await.unwrap();
    assert!(p3.is_empty(), "a delivered effect is no longer scheduled");

    let value = read(&store, ws, "outbox", "e1").await.unwrap().unwrap();
    let eff: Effect = serde_json::from_value(value).unwrap();
    assert_eq!(eff.status, EffectStatus::Delivered);
    assert_eq!(eff.attempts, 2, "two attempts: one failed, one delivered");
}

#[tokio::test]
async fn re_enqueuing_the_same_effect_id_is_idempotent() {
    // The effect id is stable, so a retried enqueue upserts one row (outbox scope). Never two PRs
    // queued for the same change.
    let store = Store::memory().await.unwrap();
    let ws = "outbox-idem";
    enqueue_pr(&store, ws, "e1", "pr:2451", 1).await;
    enqueue_pr(&store, ws, "e1", "pr:2451", 2).await; // retry

    let effs = pending(&store, ws).await.unwrap();
    assert_eq!(effs.len(), 1, "the retry upserted one effect, not two");
}

#[tokio::test]
async fn an_effect_is_invisible_across_the_workspace_wall() {
    // MANDATORY workspace-isolation (testing §2.2): a ws-B relay scan can never see a ws-A effect,
    // and a ws-B mark cannot touch a ws-A row — the namespace is the hard wall (README §7).
    let store = Store::memory().await.unwrap();
    enqueue_pr(&store, "outbox-iso-a", "e1", "pr:a", 1).await;

    // ws-B sees nothing.
    let from_b = pending(&store, "outbox-iso-b").await.unwrap();
    assert!(from_b.is_empty(), "ws-B must not see ws-A's effects");

    // ws-B cannot mark ws-A's effect (it does not exist in B's namespace).
    let err = mark_delivered(&store, "outbox-iso-b", "e1").await;
    assert!(err.is_err(), "ws-B cannot mark a ws-A effect delivered");

    // ws-A's effect is untouched — still pending.
    let from_a = pending(&store, "outbox-iso-a").await.unwrap();
    assert_eq!(from_a.len(), 1);
    assert_eq!(from_a[0].status, EffectStatus::Pending);
}
