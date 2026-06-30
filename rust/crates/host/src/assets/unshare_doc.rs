//! Revoke a share — delete the `share` relation edge (document-store scope "revoke removes
//! visibility immediately"). The mirror of `share_doc`: `store:doc/{id}:write` AND ownership.
//! Revoking is an owner act; the next `get_doc` re-resolves the live relations and the subject
//! the doc was shared to is denied.

use lb_assets::{get_doc as store_get_doc, unrelate};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_doc;
use super::error::AssetError;
use super::visibility::SHARE;

/// Revoke doc `id`'s share to `subject` (`team:…` / `user:…`) in workspace `ws`, as the doc's
/// owner. Idempotent.
pub async fn unshare_doc(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    subject: &str,
) -> Result<(), AssetError> {
    authorize_doc(principal, ws, id, Action::Write)?;
    let doc = store_get_doc(store, ws, id)
        .await?
        .ok_or(AssetError::NotFound)?;
    if doc.owner != principal.sub() {
        return Err(AssetError::Denied);
    }
    unrelate(store, ws, SHARE, id, subject).await?;
    Ok(())
}
