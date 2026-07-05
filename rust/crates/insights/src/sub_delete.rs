//! `sub_delete` — delete a subscription (insight-subscriptions-scope.md).
//!
//! Owner-or-admin only (the host checks ownership). Idempotent on an already-gone id. The notify
//! state for this sub's keys becomes orphaned; the retention follow-up sweeps it.
//!
//! **STUB**: body deferred — see the punch-list.

use crate::error::InsightsError;
use crate::subscription::TABLE;
use lb_store::{delete, Store};

/// Delete the sub at `(ws, id)`. Idempotent. The host has verified the caller owns it (or admin).
// SCOPE: docs/scope/insights/insight-subscriptions-scope.md §"Verb surface"
pub async fn sub_delete(store: &Store, ws: &str, id: &str) -> Result<(), InsightsError> {
    delete(store, ws, TABLE, id).await?;
    Ok(())
}
