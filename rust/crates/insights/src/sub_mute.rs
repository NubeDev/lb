//! `sub_mute` — toggle a subscription's `muted` flag (insight-subscriptions-scope.md).
//!
//! Owner only. A muted sub keeps matching (the matcher still produces intents for it) but the
//! notify engine skips its deliveries — accounting continues so an unmute doesn't lose the digest.
//!
//! **STUB**: body deferred — see the punch-list.

use crate::error::InsightsError;
use lb_store::Store;

/// Set the `muted` flag on sub `(ws, id)`. The host has verified the caller owns it.
// SCOPE: docs/scope/insights/insight-subscriptions-scope.md §"Verb surface" + §"The record"
pub async fn sub_mute(
    _store: &Store,
    _ws: &str,
    _id: &str,
    _muted: bool,
) -> Result<(), InsightsError> {
    // Read the row, flip `muted`, write back. If absent ⇒ BadInput.
    todo!("insights: sub mute toggle — SCOPE: subscriptions-scope.md §Verb surface")
}
