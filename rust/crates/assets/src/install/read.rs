//! Read an extension's install record by id from the workspace namespace.
//!
//! The namespace is selected from `ws`, so a read for workspace A returns `None` for an
//! extension installed only in workspace B — even the same `ext_id` (README §7). This is the
//! durable answer to "what is this extension allowed here?" the loader consults.

use lb_store::{read, Store, StoreError};

use super::delete::TOMBSTONE;
use super::model::Install;
use super::TABLE;

/// Fetch the install record for `ext_id` in workspace `ws`. `None` if it is not installed here —
/// including a tombstoned (uninstalled) row, which reads as absent (lifecycle-management scope).
pub async fn read_install(
    store: &Store,
    ws: &str,
    ext_id: &str,
) -> Result<Option<Install>, StoreError> {
    match read(store, ws, TABLE, ext_id).await? {
        Some(value) => {
            if value.get("kind").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
                return Ok(None); // uninstalled — read as absent.
            }
            let install =
                serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(install))
        }
        None => Ok(None),
    }
}
