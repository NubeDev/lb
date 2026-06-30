//! List a workspace's docs owned by a principal, oldest→newest.
//!
//! The store `list` is a namespace-scoped filter (README §7), so it can only ever return this
//! workspace's docs. Filtering by `owner` keeps the listing to the caller's own docs — the
//! host adds shared/linked docs on top by resolving relations (it owns the membership view).
//! The store filter does not order (see store `list` doc), so the `ts` sort is here.

use lb_store::{list as store_list, Store, StoreError};

use super::model::Doc;
use super::{TABLE, TOMBSTONE};

/// Return every doc in `ws` owned by `owner`, ordered by `ts` ascending. Tombstoned
/// (soft-deleted) docs are skipped. Empty if none — never another workspace's docs.
pub async fn list_docs(store: &Store, ws: &str, owner: &str) -> Result<Vec<Doc>, StoreError> {
    let rows = store_list(store, ws, TABLE, "owner", owner).await?;
    let mut docs: Vec<Doc> = Vec::new();
    for v in rows {
        if v.get("id").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
            continue;
        }
        let d: Doc = serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
        docs.push(d);
    }
    docs.sort_by_key(|d| d.ts);
    Ok(docs)
}
