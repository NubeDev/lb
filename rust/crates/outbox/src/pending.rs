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

use lb_store::{list as store_list, Store, StoreError};

use super::model::Effect;
use super::TABLE;

/// Return every undelivered effect in workspace `ws` (status `pending` or `failed`), oldest→newest.
/// Empty if none — never another workspace's effects.
pub async fn pending(store: &Store, ws: &str) -> Result<Vec<Effect>, StoreError> {
    let mut effects = Vec::new();
    for status in ["pending", "failed"] {
        let rows = store_list(store, ws, TABLE, "status", status).await?;
        for v in rows {
            effects.push(serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?);
        }
    }
    effects.sort_by_key(|e: &Effect| e.ts);
    Ok(effects)
}
