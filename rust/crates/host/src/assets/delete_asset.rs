//! Soft-delete a binary asset (document-store scope). Authorization mirrors `delete_doc`:
//! `store:asset/{id}:write` AND ownership. Tombstone upsert, idempotent.

use lb_assets::{delete_asset as store_delete_asset, get_asset as store_get_asset};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_asset;
use super::error::AssetError;

/// Soft-delete asset `id` in workspace `ws`, as the asset's owner. Idempotent.
pub async fn delete_asset(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), AssetError> {
    authorize_asset(principal, ws, id, Action::Write)?;
    // Already absent → idempotent no-op (mirrors `delete_doc`).
    let Some(asset) = store_get_asset(store, ws, id).await? else {
        return Ok(());
    };
    if asset.owner != principal.sub() {
        return Err(AssetError::Denied);
    }
    store_delete_asset(store, ws, id).await?;
    Ok(())
}
