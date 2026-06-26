//! Scan a workspace's **schedulable** effects — the relay's durable backstop (outbox scope).
//!
//! "Schedulable" = not yet `Delivered`: both `Pending` (never tried) and `Failed` (tried, still
//! owed). A LIVE query gives instant pickup, but it is ephemeral (§6.2) — this durable scan is the
//! source of truth, so a relay that restarts simply re-reads the same set and an effect that
//! crashed mid-delivery is found again (never lost). The namespace is selected from `ws`, so a
//! ws-B scan can physically only return ws-B effects (the hard wall, §7).
//!
//! The generic store `list` is a pure equality filter (it does not order — see
//! debugging/store/order-by-needs-selected-idiom.md), so this verb runs the two undelivered statuses
//! and merges them, ordering by the logical `ts` itself (deterministic — `ts` is injected, §3).
//!
//! Two reads on this set:
//!   - [`pending`] — every *schedulable* effect (pending or failed), regardless of backoff. The
//!     audit/observability view and what the existing tests assert on. A `DeadLettered` effect is
//!     terminal and never appears here.
//!   - [`due`] — the subset whose `next_attempt_ts <= now`: what the relay should actually attempt
//!     this pass. This is the backoff gate — a recently-failed effect is still `pending` (owed) but
//!     not yet `due` (waiting out its backoff).
//!
//! Plus [`dead_lettered`] — the parked poison messages, for an operator to inspect / replay.

use lb_store::{list as store_list, Store, StoreError};

use super::model::Effect;
use super::TABLE;

/// Return every undelivered (schedulable) effect in workspace `ws` (status `pending` or `failed`),
/// oldest→newest. Excludes `DeadLettered` (terminal). Empty if none — never another workspace's.
pub async fn pending(store: &Store, ws: &str) -> Result<Vec<Effect>, StoreError> {
    let mut effects = Vec::new();
    for status in ["pending", "failed"] {
        for v in store_list(store, ws, TABLE, "status", status).await? {
            effects.push(serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?);
        }
    }
    effects.sort_by_key(|e: &Effect| e.ts);
    Ok(effects)
}

/// The effects the relay should attempt at logical time `now`: schedulable AND past their backoff
/// gate (`next_attempt_ts <= now`). A failed effect waiting out its backoff is owed but not yet due.
pub async fn due(store: &Store, ws: &str, now: u64) -> Result<Vec<Effect>, StoreError> {
    let mut effects = pending(store, ws).await?;
    effects.retain(|e| e.next_attempt_ts <= now);
    Ok(effects)
}

/// The dead-lettered (poison) effects in workspace `ws` — exhausted `max_attempts`, parked for
/// audit/replay, never re-delivered by the relay. Oldest→newest.
pub async fn dead_lettered(store: &Store, ws: &str) -> Result<Vec<Effect>, StoreError> {
    let mut effects = Vec::new();
    for v in store_list(store, ws, TABLE, "status", "dead-lettered").await? {
        effects.push(serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?);
    }
    effects.sort_by_key(|e: &Effect| e.ts);
    Ok(effects)
}
