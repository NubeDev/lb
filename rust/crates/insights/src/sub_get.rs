//! `sub_get` — read one subscription by id (insight-subscriptions-scope.md).
//!
//! Owner-or-admin only (the host checks ownership before delegating here). Returns `None` if the
//! id does not exist IN THIS WORKSPACE (the namespace is the hard wall — README §7).
//!
//! **STUB**: body deferred — see the punch-list.

use crate::error::InsightsError;
use crate::subscription::Subscription;
use lb_store::Store;

/// Return the sub at `(ws, id)`, or `None` if absent in this workspace.
// SCOPE: docs/scope/insights/insight-subscriptions-scope.md §"Verb surface"
pub async fn sub_get(
    _store: &Store,
    _ws: &str,
    _id: &str,
) -> Result<Option<Subscription>, InsightsError> {
    // `read(store, ws, TABLE, id)` and decode, or `None` if the store returned None.
    todo!("insights: sub get — SCOPE: subscriptions-scope.md §Verb surface")
}
