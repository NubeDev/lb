//! Delete an install record — the durable half of `ext.uninstall` (lifecycle-management scope). The
//! store has no row-delete, so this upserts a tombstone (kind ≠ `install`), so `list_installs` and
//! `read_install` read it as absent — sync-idempotent (§6.8), like every other tombstone here.

use lb_store::{write, Store, StoreError};

use super::TABLE;

/// The kind a deleted (uninstalled) install carries — excluded by `list_installs`'s kind filter.
pub(crate) const TOMBSTONE: &str = "__uninstalled__";

/// Tombstone the install record for `ext_id` in workspace `ws`. Idempotent; uninstalling an absent
/// extension is a no-op success, never a cross-workspace reach.
pub async fn delete_install(store: &Store, ws: &str, ext_id: &str) -> Result<(), StoreError> {
    let tombstone = serde_json::json!({ "kind": TOMBSTONE, "ext_id": ext_id });
    write(store, ws, TABLE, ext_id, &tombstone).await
}
