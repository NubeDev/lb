//! Read a single document asset by id from the workspace namespace.
//!
//! The namespace is selected from `ws`, so a read for workspace A returns `None` for a doc
//! that lives in workspace B — even with the same id (README §7). Raw verb: the host resolves
//! visibility (owner / shared team / linked channel) *before* calling this; this just fetches.

use lb_store::{read, Store, StoreError};

use super::model::Doc;
use super::TABLE;

/// Fetch `doc:{id}` from workspace `ws`. `None` if absent in *this* namespace.
pub async fn get_doc(store: &Store, ws: &str, id: &str) -> Result<Option<Doc>, StoreError> {
    match read(store, ws, TABLE, id).await? {
        Some(value) => {
            let doc =
                serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(doc))
        }
        None => Ok(None),
    }
}
