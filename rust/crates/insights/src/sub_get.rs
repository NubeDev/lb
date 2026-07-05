//! `sub_get` — read one subscription by id (insight-subscriptions-scope.md).
//!
//! Owner-or-admin only (the host checks ownership before delegating here). Returns `None` if the
//! id does not exist IN THIS WORKSPACE (the namespace is the hard wall — README §7).
//!
//! **STUB**: body deferred — see the punch-list.

use crate::error::InsightsError;
use crate::subscription::{Subscription, TABLE};
use lb_store::{read, Store};

/// Return the sub at `(ws, id)`, or `None` if absent in this workspace.
// SCOPE: docs/scope/insights/insight-subscriptions-scope.md §"Verb surface"
pub async fn sub_get(
    store: &Store,
    ws: &str,
    id: &str,
) -> Result<Option<Subscription>, InsightsError> {
    let Some(value) = read(store, ws, TABLE, id).await? else {
        return Ok(None);
    };
    let sub: Subscription = serde_json::from_value(value)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    Ok(Some(sub))
}
