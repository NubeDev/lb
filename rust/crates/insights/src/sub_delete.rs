//! `sub_delete` — delete a subscription (insight-subscriptions-scope.md).
//!
//! Owner-or-admin only (the host checks ownership). Idempotent on an already-gone id. The notify
//! state for this sub's keys becomes orphaned; the retention follow-up sweeps it.
//!
//! **STUB**: body deferred — see the punch-list.

use crate::error::InsightsError;
use lb_store::Store;

/// Delete the sub at `(ws, id)`. Idempotent. The host has verified the caller owns it (or admin).
// SCOPE: docs/scope/insights/insight-subscriptions-scope.md §"Verb surface"
pub async fn sub_delete(_store: &Store, _ws: &str, _id: &str) -> Result<(), InsightsError> {
    // `delete(store, ws, TABLE, id)` — idempotent (a missing row is success).
    todo!("insights: sub delete (idempotent) — SCOPE: subscriptions-scope.md §Verb surface")
}
