//! Persist (upsert) a document asset into the workspace namespace.
//!
//! Idempotent on the doc `id`: re-`put` overwrites the same `doc:{id}` row (docs are
//! mutable-by-upsert at S4 — versioning is a skill concern, files scope non-goal). The
//! namespace is selected from `ws` by `lb_store`, so a write can only land in its own
//! workspace (README §7). Raw verb — authorization is the host's job.

use lb_store::{write, Store, StoreError};

use super::model::Doc;
use super::TABLE;

/// Upsert `doc` into workspace `ws`'s doc table. Idempotent on `doc.id`.
pub async fn put_doc(store: &Store, ws: &str, doc: &Doc) -> Result<(), StoreError> {
    let value = serde_json::to_value(doc).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &doc.id, &value).await
}
