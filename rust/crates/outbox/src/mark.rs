//! Record the outcome of a relay delivery attempt — `mark_delivered` (acknowledged) and
//! `mark_failed` (the attempt failed). Both load the `outbox:{id}` row, mutate it, and upsert it
//! back, workspace-namespaced (the hard wall, §7).
//!
//! `mark_failed` is where backoff + dead-letter live (the outbox scope's deferred question, now
//! answered): it counts the attempt, and then either
//!   - **dead-letters** the effect (status `DeadLettered`) if it has now reached `max_attempts` — a
//!     poison message stops retrying and is parked for audit; or
//!   - leaves it `Failed` but pushes `next_attempt_ts` out by `backoff(attempts)`, so the relay
//!     waits longer before each retry instead of hammering a down target every pass.
//!
//! A `Failed` effect past its `next_attempt_ts` is still returned by [`pending`](super::pending), so
//! the at-least-once retry holds; a `DeadLettered` effect is not (it is terminal). `mark_delivered`
//! is the only transition that stops re-delivery on success (the receiver's `idempotency_key` dedup
//! covers the race where it acknowledged but we crashed before marking).
//!
//! Raw verbs — the relay (host) authorizes/owns the loop; these just persist the outcome.

use lb_store::{read, write, Store, StoreError};

use super::model::{backoff, Effect, EffectStatus};
use super::TABLE;

/// Mark effect `id` in workspace `ws` as `Delivered` and count the attempt. Errors if the effect
/// is absent here (a mark for a missing or cross-workspace effect is a bug, not a silent create).
pub async fn mark_delivered(store: &Store, ws: &str, id: &str) -> Result<(), StoreError> {
    update(store, ws, id, |e| {
        e.status = EffectStatus::Delivered;
        e.attempts += 1;
        // Clear a previous attempt's reason: the row's last_error must describe its CURRENT state, and
        // a delivered effect with a stale error reads as a failure to whoever is triaging.
        e.last_error = None;
    })
    .await
}

/// Record a **transient** failed delivery of effect `id` in workspace `ws` at logical time `now`.
/// Counts the attempt, records `reason` on the row, then dead-letters the effect if it has hit
/// `max_attempts`, else schedules the next retry at `now + backoff(attempts)`. Errors if the effect is
/// absent here. Returns the effect's status after the update (so the relay can tally dead-letters
/// without a re-read).
///
/// `reason` must already be sanitized by the caller — it is durable, operator-visible text.
pub async fn mark_failed(
    store: &Store,
    ws: &str,
    id: &str,
    now: u64,
    reason: &str,
) -> Result<EffectStatus, StoreError> {
    let mut resulting = EffectStatus::Failed;
    update(store, ws, id, |e| {
        e.attempts += 1;
        e.last_error = Some(reason.to_string());
        if e.attempts >= e.max_attempts {
            e.status = EffectStatus::DeadLettered;
        } else {
            e.status = EffectStatus::Failed;
            e.next_attempt_ts = now.saturating_add(backoff(e.attempts));
        }
        resulting = e.status;
    })
    .await?;
    Ok(resulting)
}

/// Record a **permanent** delivery failure of effect `id` — park it immediately, with no further
/// attempts, and record `reason` for the operator.
///
/// The distinction from [`mark_failed`] is the whole point (email-transport scope, "Delivery outcome is
/// honest"): a target that *knows* retrying cannot help — `550 no such mailbox`, a revoked OAuth grant,
/// a message with no recipient — should not be retried five times with backoff. Retrying a permanent
/// failure is not merely wasted work: it delays the dead-letter row that tells an operator to fix
/// something, and against a rate-limiting relay it earns a reputation penalty for a mistake that will
/// never resolve.
///
/// Terminal, exactly like an exhausted retry budget: [`pending`](super::pending) does not return a
/// dead-lettered effect, and the row is kept for audit and manual replay.
pub async fn mark_dead_lettered(
    store: &Store,
    ws: &str,
    id: &str,
    reason: &str,
) -> Result<EffectStatus, StoreError> {
    update(store, ws, id, |e| {
        e.attempts += 1;
        e.status = EffectStatus::DeadLettered;
        e.last_error = Some(reason.to_string());
    })
    .await?;
    Ok(EffectStatus::DeadLettered)
}

/// Load `outbox:{id}` in `ws`, apply `mutate`, and upsert it back. The one read-modify-write seam
/// both marks share, so the status/attempt bookkeeping lives in exactly one place.
async fn update(
    store: &Store,
    ws: &str,
    id: &str,
    mutate: impl FnOnce(&mut Effect),
) -> Result<(), StoreError> {
    let value = read(store, ws, TABLE, id)
        .await?
        .ok_or_else(|| StoreError::Decode(format!("mark: no effect {id} in ws {ws}")))?;
    let mut effect: Effect =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    mutate(&mut effect);
    let updated = serde_json::to_value(&effect).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, id, &updated).await
}
