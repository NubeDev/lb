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
    })
    .await
}

/// Record a failed delivery of effect `id` in workspace `ws` at logical time `now`. Counts the
/// attempt, then dead-letters the effect if it has hit `max_attempts`, else schedules the next retry
/// at `now + backoff(attempts)`. Errors if the effect is absent here. Returns the effect's status
/// after the update (so the relay can tally dead-letters without a re-read).
pub async fn mark_failed(
    store: &Store,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<EffectStatus, StoreError> {
    let mut resulting = EffectStatus::Failed;
    update(store, ws, id, |e| {
        e.attempts += 1;
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
