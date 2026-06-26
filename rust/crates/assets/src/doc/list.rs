//! List a workspace's docs owned by a principal, oldest→newest.
//!
//! The store `list` is a namespace-scoped filter (README §7), so it can only ever return this
//! workspace's docs. Filtering by `owner` keeps the listing to the caller's own docs — the
//! host adds shared/linked docs on top by resolving relations (it owns the membership view).
//! The store filter does not order (see store `list` doc), so the `ts` sort is here.

use lb_store::{list as store_list, Store, StoreError};

use super::model::Doc;
use super::TABLE;

/// Return every doc in `ws` owned by `owner`, ordered by `ts` ascending. Empty if none —
/// never another workspace's docs.
pub async fn list_docs(store: &Store, ws: &str, owner: &str) -> Result<Vec<Doc>, StoreError> {
    let rows = store_list(store, ws, TABLE, "owner", owner).await?;
    let mut docs: Vec<Doc> = rows
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect::<Result<_, _>>()?;
    docs.sort_by_key(|d| d.ts);
    Ok(docs)
}
