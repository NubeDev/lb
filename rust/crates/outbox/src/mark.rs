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
//! **The supersede guard (`delivered_ts`).** A stable-id effect (a setpoint keyed per point+slot,
//! deliberately, so rapid re-writes coalesce) can be re-enqueued with a NEWER payload while an older
//! version's delivery is still in flight. A mark keyed on id alone would then stamp the new,
//! never-sent payload with the old attempt's outcome — for `Delivered` that is a lost update (the
//! box keeps the old value forever; found live as "slide 4 times, the box settles on the middle
//! one"). Every mark therefore takes the `ts` of the effect version the relay actually pulled: a
//! mark whose `ts` no longer matches the row is a **no-op** — the newer version stays `Pending` and
//! the next relay pass delivers it. `None` marks unconditionally (an operator replay / a caller
//! that knows the id is never re-enqueued).
//!
//! Raw verbs — the relay (host) authorizes/owns the loop; these just persist the outcome.

use lb_store::{read, write, Store, StoreError};

use super::model::{backoff, Effect, EffectStatus};
use super::TABLE;

/// Mark effect `id` in workspace `ws` as `Delivered` and count the attempt. Errors if the effect
/// is absent here (a mark for a missing or cross-workspace effect is a bug, not a silent create).
/// `delivered_ts`: the supersede guard (module doc) — superseded marks are silent no-ops.
pub async fn mark_delivered(
    store: &Store,
    ws: &str,
    id: &str,
    delivered_ts: Option<u64>,
) -> Result<(), StoreError> {
    update(store, ws, id, delivered_ts, |e| {
        e.status = EffectStatus::Delivered;
        e.attempts += 1;
        // Clear a previous attempt's reason: the row's last_error must describe its CURRENT state, and
        // a delivered effect with a stale error reads as a failure to whoever is triaging.
        e.last_error = None;
    })
    .await
    .map(|_| ())
}

/// Record a **transient** failed delivery of effect `id` in workspace `ws` at logical time `now`.
/// Counts the attempt, records `reason` on the row, then dead-letters the effect if it has hit
/// `max_attempts`, else schedules the next retry at `now + backoff(attempts)`. Errors if the effect is
/// absent here. Returns the effect's status after the update (so the relay can tally dead-letters
/// without a re-read). `delivered_ts`: the supersede guard (module doc) — a superseded failure
/// leaves the newer pending version untouched (no attempt counted, no backoff) and returns its
/// current status.
///
/// `reason` must already be sanitized by the caller — it is durable, operator-visible text.
pub async fn mark_failed(
    store: &Store,
    ws: &str,
    id: &str,
    now: u64,
    reason: &str,
    delivered_ts: Option<u64>,
) -> Result<EffectStatus, StoreError> {
    let effect = update(store, ws, id, delivered_ts, |e| {
        e.attempts += 1;
        e.last_error = Some(reason.to_string());
        if e.attempts >= e.max_attempts {
            e.status = EffectStatus::DeadLettered;
        } else {
            e.status = EffectStatus::Failed;
            e.next_attempt_ts = now.saturating_add(backoff(e.attempts));
        }
    })
    .await?;
    Ok(effect.status)
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
/// dead-lettered effect, and the row is kept for audit and manual replay. `delivered_ts`: the
/// supersede guard (module doc) — a newer pending version must not be parked for the OLD version's
/// permanent failure (its payload may be deliverable), so a superseded park is a no-op too.
pub async fn mark_dead_lettered(
    store: &Store,
    ws: &str,
    id: &str,
    reason: &str,
    delivered_ts: Option<u64>,
) -> Result<EffectStatus, StoreError> {
    let effect = update(store, ws, id, delivered_ts, |e| {
        e.attempts += 1;
        e.status = EffectStatus::DeadLettered;
        e.last_error = Some(reason.to_string());
    })
    .await?;
    Ok(effect.status)
}

/// Load `outbox:{id}` in `ws`, apply `mutate`, and upsert it back — UNLESS `delivered_ts` says the
/// row has been re-enqueued since the caller pulled it (the supersede guard, module doc), in which
/// case the row is returned untouched. The one read-modify-write seam every mark shares, so the
/// status/attempt bookkeeping AND the guard live in exactly one place.
async fn update(
    store: &Store,
    ws: &str,
    id: &str,
    delivered_ts: Option<u64>,
    mutate: impl FnOnce(&mut Effect),
) -> Result<Effect, StoreError> {
    let value = read(store, ws, TABLE, id)
        .await?
        .ok_or_else(|| StoreError::Decode(format!("mark: no effect {id} in ws {ws}")))?;
    let mut effect: Effect =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    if let Some(ts) = delivered_ts {
        if effect.ts != ts {
            return Ok(effect); // superseded — a newer version owns this row now
        }
    }
    mutate(&mut effect);
    let updated = serde_json::to_value(&effect).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, id, &updated).await?;
    Ok(effect)
}
