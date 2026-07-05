//! `sub_mute` — toggle a subscription's `muted` flag (insight-subscriptions-scope.md).
//!
//! Owner only. A muted sub keeps matching (the matcher still produces intents for it) but the
//! notify engine skips its deliveries — accounting continues so an unmute doesn't lose the digest.
//!
//! **STUB**: body deferred — see the punch-list.

use crate::error::InsightsError;
use crate::subscription::TABLE;
use lb_store::{write, Store};

/// Set the `muted` flag on sub `(ws, id)`. The host has verified the caller owns it.
// SCOPE: docs/scope/insights/insight-subscriptions-scope.md §"Verb surface" + §"The record"
pub async fn sub_mute(store: &Store, ws: &str, id: &str, muted: bool) -> Result<(), InsightsError> {
    let Some(mut sub) = crate::sub_get::sub_get(store, ws, id).await? else {
        return Err(InsightsError::BadInput(format!(
            "no such subscription: {id}"
        )));
    };
    sub.muted = muted;
    let value = serde_json::to_value(&sub)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    write(store, ws, TABLE, id, &value).await?;
    Ok(())
}
