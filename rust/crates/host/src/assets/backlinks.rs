//! Backlinks — "what links here?" (document-store scope move 3). Returns the ids of every doc
//! that carries an internal `lb-doc://{id}` reference to `id`, i.e. every `a` where the
//! `doclink` edge `a -[doclink]-> id` exists. The inverse of the relation listing, backed by
//! the denormalized `bpair` filter (`lb_assets::list_related_inverse`).
//!
//! Authorization: `store:doc/{id}:read` — backlinks to a doc the caller cannot read are denied
//! (gate 3 re-resolved via `may_read_doc`), so the listing never leaks which docs reference a
//! doc the caller lacks. The returned ids are NOT pre-filtered for the caller's read on each
//! *source* doc (a source the caller can't read is still named — backlinks are a navigation
//! aid, not content; the link target's content is re-gated when the caller follows it).

use lb_assets::{get_doc as store_get_doc, list_related_inverse};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_doc;
use super::error::AssetError;
use super::visibility::{may_read_doc, DOCLINK};

/// Return the ids of every doc linking to doc `id` in workspace `ws`. Denied if the caller
/// cannot read `id` itself (gate 3).
pub async fn backlinks(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Vec<String>, AssetError> {
    authorize_doc(principal, ws, id, Action::Read)?;
    let doc = store_get_doc(store, ws, id)
        .await?
        .ok_or(AssetError::NotFound)?;
    may_read_doc(store, principal, ws, &doc).await?;
    Ok(list_related_inverse(store, ws, DOCLINK, id).await?)
}
