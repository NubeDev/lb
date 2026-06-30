//! Soft-delete a doc — the document-store scope's idempotent tombstone (move "CRUD: delete_doc
//! soft-delete tombstone, idempotent"). Authorization mirrors `share_doc`: `store:doc/{id}:write`
//! AND ownership — only the owner deletes their doc (a wildcard writer cannot remove someone
//! else's). The tombstone upsert is §6.8-sync-friendly and `get_doc` reads it as absent.

use lb_assets::{delete_doc as store_delete_doc, get_doc as store_get_doc};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_doc;
use super::error::AssetError;

/// Soft-delete doc `id` in workspace `ws`, as the doc's owner. Idempotent.
pub async fn delete_doc(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), AssetError> {
    authorize_doc(principal, ws, id, Action::Write)?;
    // Already absent (never created, or already soft-deleted) → idempotent no-op. The scope
    // requires delete to be idempotent; a tombstone reads as `None` (see `lb_assets::get_doc`).
    let Some(doc) = store_get_doc(store, ws, id).await? else {
        return Ok(());
    };
    if doc.owner != principal.sub() {
        return Err(AssetError::Denied);
    }
    store_delete_doc(store, ws, id).await?;
    Ok(())
}
