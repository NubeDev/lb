//! Delete a binary asset — a soft-delete tombstone upsert (idempotent, §6.8-sync-friendly),
//! mirroring the relation revoke (`relation::unrelate`). A delete never vanishes a row under a
//! peer; `get_asset` reads a tombstone as `None`. Raw verb — authorization is the host's job.

use lb_store::{write, Store, StoreError};
use serde_json::json;

use super::model::TOMBSTONE;
use super::TABLE;

/// Soft-delete `asset:{id}` in workspace `ws`. Idempotent.
pub async fn delete_asset(store: &Store, ws: &str, id: &str) -> Result<(), StoreError> {
    let tombstone = json!({ "id": TOMBSTONE });
    write(store, ws, TABLE, id, &tombstone).await
}
