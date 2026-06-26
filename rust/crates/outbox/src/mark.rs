//! Record the outcome of a relay delivery attempt — `mark_delivered` (acknowledged) and
//! `mark_failed` (the attempt failed; the effect stays schedulable). Both load the `outbox:{id}`
//! row, mutate it, and upsert it back, workspace-namespaced (the hard wall, §7).
//!
//! A `Failed` effect is still returned by [`pending`](super::pending), so the next relay pass
//! re-delivers it — the at-least-once retry (outbox scope). `mark_delivered` is the only transition
//! that stops re-delivery, so once a target acknowledges, no second send happens (the receiver's
//! `idempotency_key` dedup covers the race where it acknowledged but we crashed before marking).
//!
//! Raw verbs — the relay (host) authorizes/owns the loop; these just persist the outcome.

use lb_store::{read, write, Store, StoreError};

use super::model::{Effect, EffectStatus};
use super::TABLE;

/// Mark effect `id` in workspace `ws` as `Delivered` and count the attempt. Errors if the effect
/// is absent here (a mark for a missing or cross-workspace effect is a bug, not a silent create).
pub async fn mark_delivered(store: &Store, ws: &str, id: &str) -> Result<(), StoreError> {
    set_status(store, ws, id, EffectStatus::Delivered).await
}

/// Mark effect `id` in workspace `ws` as `Failed` and count the attempt — it stays schedulable, so
/// the next relay pass re-delivers it (the retry path). Errors if the effect is absent here.
pub async fn mark_failed(store: &Store, ws: &str, id: &str) -> Result<(), StoreError> {
    set_status(store, ws, id, EffectStatus::Failed).await
}

async fn set_status(
    store: &Store,
    ws: &str,
    id: &str,
    status: EffectStatus,
) -> Result<(), StoreError> {
    let value = read(store, ws, TABLE, id)
        .await?
        .ok_or_else(|| StoreError::Decode(format!("mark: no effect {id} in ws {ws}")))?;
    let mut effect: Effect =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    effect.status = status;
    effect.attempts += 1;
    let updated = serde_json::to_value(&effect).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, id, &updated).await
}
