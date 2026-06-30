//! Read a single binary asset by id from the workspace namespace. `None` for an absent record
//! OR a tombstoned (deleted) one — a delete is an upsert-to-tombstone (`delete_asset`). Raw
//! verb: the host resolves access (owner / shared / embedder) *before* calling this.

use lb_store::{read, Store, StoreError};
use serde_json::Value;

use super::model::{Asset, TOMBSTONE};
use super::TABLE;

/// Fetch `asset:{id}` from workspace `ws`. `None` if absent or tombstoned in this namespace.
pub async fn get_asset(store: &Store, ws: &str, id: &str) -> Result<Option<Asset>, StoreError> {
    match read(store, ws, TABLE, id).await? {
        Some(value) => {
            if is_tombstone(&value) {
                return Ok(None);
            }
            let asset =
                serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(asset))
        }
        None => Ok(None),
    }
}

fn is_tombstone(value: &Value) -> bool {
    value.get("id").and_then(|k| k.as_str()) == Some(TOMBSTONE)
}
