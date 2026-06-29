//! Read a reminder back by id — the `get`/reactor read. `None` if absent in *this* namespace (the
//! hard wall, §7 — a cross-workspace read returns `None`, never another workspace's reminder).

use lb_store::{read, Store, StoreError};

use crate::model::Reminder;

/// Fetch `reminder:{id}` from workspace `ws`. `None` if absent in this namespace.
pub async fn load(store: &Store, ws: &str, id: &str) -> Result<Option<Reminder>, StoreError> {
    let Some(value) = read(store, ws, super::TABLE, id).await? else {
        return Ok(None);
    };
    let reminder: Reminder =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(Some(reminder))
}
