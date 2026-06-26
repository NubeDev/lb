//! Persist an extension install record into the workspace namespace.
//!
//! Idempotent on `ext_id` (re-installing/upgrading upserts the same row with the new version +
//! granted set). Namespace-scoped (README §7); raw verb the host loader calls after computing
//! the grant intersection.

use lb_store::{write, Store, StoreError};

use super::model::Install;
use super::TABLE;

/// Upsert the install record for `install.ext_id` into workspace `ws`.
pub async fn record_install(store: &Store, ws: &str, install: &Install) -> Result<(), StoreError> {
    let value = serde_json::to_value(install).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &install.ext_id, &value).await
}
