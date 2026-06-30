//! Persist (upsert) a binary asset into the workspace namespace. Idempotent on `asset.id`.
//! Raw verb — size bounding and authorization are the host's job (`put_asset`).

use lb_store::{write, Store, StoreError};

use super::model::Asset;
use super::TABLE;

/// Upsert `asset` into workspace `ws`'s asset table. Idempotent on `asset.id`.
pub async fn put_asset(store: &Store, ws: &str, asset: &Asset) -> Result<(), StoreError> {
    let value = serde_json::to_value(asset).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &asset.id, &value).await
}
