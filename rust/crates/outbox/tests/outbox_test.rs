//! The store-layer guarantees the must-deliver outbox leans on (outbox scope, testing §2): the
//! transactional enqueue is atomic, the relay's pending scan is a durable backstop, re-delivery is
//! idempotent, and the workspace wall holds. Pure store verbs — no node/bus — so a plain
//! `tokio::test` (no Zenoh peer) is enough.

use lb_outbox::{
    backoff, dead_lettered, due, enqueue, mark_delivered, mark_failed, pending, Effect,
    EffectStatus,
};
use lb_store::{read, Store};
use serde_json::json;

/// A domain change + an effect, enqueued together.
async fn enqueue_pr(store: &Store, ws: &str, eff_id: &str, key: &str, ts: u64) {
    enqueue_pr_capped(store, ws, eff_id, key, ts, None).await;
}

/// As [`enqueue_pr`], but optionally cap `max_attempts` (for the dead-letter test).
async fn enqueue_pr_capped(
    store: &Store,
    ws: &str,
    eff_id: &str,
    key: &str,
    ts: u64,
    max_attempts: Option<u32>,
) {
    let change = json!({ "kind": "job-step", "note": "drafted PR" });
    let mut effect = Effect::new(
        eff_id,
        "github",
        "create_pr",
        r#"{"repo":"acme/api","head":"fix/2451"}"#,
        key,
        ts,
    );
    if let Some(m) = max_attempts {
        effect = effect.with_max_attempts(m);
    }
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

    // Pass 1 at now=1: the target is down → mark failed. The effect is still owed.
    let p1 = pending(&store, ws).await.unwrap();
    assert_eq!(p1.len(), 1);
    let status = mark_failed(&store, ws, "e1", 1).await.unwrap();
    assert_eq!(
        status,
        EffectStatus::Failed,
        "one failure is not yet poison"
    );

    // It is STILL pending-for-the-relay (failed is schedulable), now with attempts counted.
    let p2 = pending(&store, ws).await.unwrap();
    assert_eq!(p2.len(), 1, "a failed effect re-appears for re-delivery");
    assert_eq!(p2[0].status, EffectStatus::Failed);
    assert_eq!(p2[0].attempts, 1, "the failed attempt was counted");

    // Pass 2 succeeds (at a now past the backoff) → delivered. Out of the schedulable set.
    let now2 = 1 + backoff(1);
    assert_eq!(
        due(&store, ws, now2).await.unwrap().len(),
        1,
        "due once backoff elapsed"
    );
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
async fn a_failed_effect_waits_out_its_backoff_before_it_is_due() {
    // BACKOFF: after a failure at now=10, the effect is still owed (`pending`) but NOT yet due — a
    // relay pass before `now + backoff(1)` skips it, so a tight retry loop does not hammer a down
    // target. Once `now` reaches the gate, it is due again.
    let store = Store::memory().await.unwrap();
    let ws = "outbox-backoff";
    enqueue_pr(&store, ws, "e1", "pr:2451", 10).await;

    mark_failed(&store, ws, "e1", 10).await.unwrap();
    let gate = 10 + backoff(1); // the earliest ts the relay may retry

    // Still schedulable (owed)…
    assert_eq!(pending(&store, ws).await.unwrap().len(), 1);
    // …but NOT due one tick before the gate.
    assert!(
        due(&store, ws, gate - 1).await.unwrap().is_empty(),
        "an effect inside its backoff window is not due"
    );
    // Due again at the gate.
    assert_eq!(
        due(&store, ws, gate).await.unwrap().len(),
        1,
        "the effect is due once its backoff has elapsed"
    );
}

#[tokio::test]
async fn an_effect_dead_letters_after_exhausting_max_attempts() {
    // DEAD-LETTER: a poison effect (the target always fails) stops retrying after `max_attempts` and
    // is parked — no longer schedulable, but kept for audit via `dead_lettered`. Cap at 3 so the
    // test is short; the last failure flips it terminal.
    let store = Store::memory().await.unwrap();
    let ws = "outbox-deadletter";
    enqueue_pr_capped(&store, ws, "e1", "pr:2451", 1, Some(3)).await;

    // Two failures: still Failed (schedulable), backoff each time.
    assert_eq!(
        mark_failed(&store, ws, "e1", 1).await.unwrap(),
        EffectStatus::Failed
    );
    assert_eq!(
        mark_failed(&store, ws, "e1", 100).await.unwrap(),
        EffectStatus::Failed
    );
    assert_eq!(
        pending(&store, ws).await.unwrap().len(),
        1,
        "still owed before the cap"
    );

    // The third failure hits max_attempts → dead-lettered (terminal).
    assert_eq!(
        mark_failed(&store, ws, "e1", 200).await.unwrap(),
        EffectStatus::DeadLettered
    );
    assert!(
        pending(&store, ws).await.unwrap().is_empty(),
        "a dead-lettered effect is no longer scheduled"
    );
    assert!(
        due(&store, ws, u64::MAX).await.unwrap().is_empty(),
        "and never becomes due, even far in the future"
    );

    // But it is kept for audit/replay.
    let parked = dead_lettered(&store, ws).await.unwrap();
    assert_eq!(parked.len(), 1, "the poison effect is parked, not deleted");
    assert_eq!(parked[0].id, "e1");
    assert_eq!(parked[0].attempts, 3);
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
