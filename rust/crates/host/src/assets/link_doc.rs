//! Link a doc into a channel — write a `link` relation edge (files scope). Anyone who may `sub`
//! the channel can then read the doc (the channel-audience read path, resolved by `may_read_doc`).
//!
//! Authorization mirrors `share_doc`: `store:doc/{id}:write` AND ownership of the doc. We do not
//! *also* require a channel grant to link (the owner chooses where their doc goes); the *reader*
//! still needs the channel `sub` grant to actually read it through the link — so no capability
//! is widened by linking.

use lb_assets::{get_doc as store_get_doc, relate};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_doc;
use super::error::AssetError;
use super::visibility::LINK;

/// Link doc `id` into channel `cid` in workspace `ws`, as the doc's owner. Idempotent.
pub async fn link_doc(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    cid: &str,
) -> Result<(), AssetError> {
    authorize_doc(principal, ws, id, Action::Write)?;
    let doc = store_get_doc(store, ws, id)
        .await?
        .ok_or(AssetError::NotFound)?;
    if doc.owner != principal.sub() {
        return Err(AssetError::Denied);
    }
    relate(store, ws, LINK, id, cid).await?;
    Ok(())
}
