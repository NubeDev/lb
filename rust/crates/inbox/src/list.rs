//! List the items of one channel from the durable inbox, oldest→newest.
//!
//! This is the read side of a channel view: the store query is namespace-scoped, so a list
//! for workspace A can only ever return A's items (README §7). Authorization (a `bus:` sub
//! grant, or an `inbox` read grant) is the caller's job — run before this raw verb.

use lb_store::{list as store_list, Store, StoreError};

use crate::item::Item;
use crate::record::TABLE;

/// Return every item in `(ws, channel)` ordered by `ts` ascending. Empty if the channel has
/// no items in *this* workspace — never another workspace's items.
///
/// The store `list` is a pure filter (it does not order — see
/// debugging/store/order-by-needs-selected-idiom.md); the inbox owns the `Item` shape, so it
/// sorts by the logical `ts` here. The sort is deterministic — `ts` is caller-injected, not
/// wall-clock (testing §3).
pub async fn list(store: &Store, ws: &str, channel: &str) -> Result<Vec<Item>, StoreError> {
    let rows = store_list(store, ws, TABLE, "channel", channel).await?;
    let mut items: Vec<Item> = rows
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect::<Result<_, _>>()?;
    items.sort_by_key(|i| i.ts);
    Ok(items)
}
