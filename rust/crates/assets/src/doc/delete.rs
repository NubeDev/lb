//! Soft-delete a document asset — a tombstone upsert (idempotent, §6.8-sync-friendly), the
//! doc analog of `relation::unrelate` / `asset::delete_asset`. `get_doc` reads a tombstone as
//! `None`; `list_docs` skips it. A delete never vanishes a row under a peer. Raw verb — the
//! host authorizes (owner act) before calling this.

use lb_store::{write, Store, StoreError};
use serde_json::json;

use super::TABLE;

/// The marker a deleted doc carries. `get_doc` / `list_docs` treat it as absent.
pub(crate) const TOMBSTONE: &str = "__deleted__";

/// Soft-delete `doc:{id}` in workspace `ws`. Idempotent.
pub async fn delete_doc(store: &Store, ws: &str, id: &str) -> Result<(), StoreError> {
    let tombstone = json!({ "id": TOMBSTONE });
    write(store, ws, TABLE, id, &tombstone).await
}
