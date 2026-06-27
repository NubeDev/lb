//! List every install record in a workspace — the union both tiers share (lifecycle-management
//! scope's `ext.list`). Backed by the constant `kind` discriminant so it is one namespace-scoped
//! `store::list` (README §7); a tombstoned (uninstalled) row carries a different kind and is excluded.

use lb_store::{list as store_list, Store, StoreError};

use super::model::{Install, KIND};
use super::TABLE;

/// Return every install in workspace `ws` (both tiers). Empty if none — never another workspace's
/// installs (the namespace wall, §7). Order is unspecified (the caller sorts for the table).
pub async fn list_installs(store: &Store, ws: &str) -> Result<Vec<Install>, StoreError> {
    let rows = store_list(store, ws, TABLE, "kind", KIND).await?;
    rows.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect()
}
