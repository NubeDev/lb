//! Read a single item by `(channel, id)` from the durable inbox.
//!
//! The store read is namespace-scoped, so a get for workspace A can only ever read A's items
//! (README §7). Authorization is the caller's job — run before this raw verb. `None` if no item
//! lives at `(ws, channel, id)` in *this* workspace — never another workspace's item.

use lb_store::{read, Store, StoreError};

use crate::item::Item;
use crate::record::{record_id, TABLE};

/// Return the item at `(ws, channel, id)`, or `None` if absent in this workspace.
pub async fn get(
    store: &Store,
    ws: &str,
    channel: &str,
    id: &str,
) -> Result<Option<Item>, StoreError> {
    let Some(value) = read(store, ws, TABLE, &record_id(channel, id)).await? else {
        return Ok(None);
    };
    let item: Item =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(Some(item))
}
