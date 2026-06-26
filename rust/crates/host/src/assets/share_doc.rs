//! Share a doc to a team — write a `share` relation edge (files scope). The owner makes their
//! own doc readable by a team's members. Authorization: `store:doc/{id}:write` (the doc surface,
//! workspace-first) AND ownership — sharing is an owner act, not something any writer of any doc
//! may do to someone else's doc.
//!
//! Sharing is a *relation*, never a content copy: a later `unshare` (revoke) is one `unrelate`,
//! and the doc instantly stops being visible to the team on the next `get_doc`.

use lb_assets::{get_doc as store_get_doc, relate};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_doc;
use super::error::AssetError;
use super::visibility::SHARE;

/// Share doc `id` to `team` in workspace `ws`, as the doc's owner. Idempotent.
pub async fn share_doc(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    team: &str,
) -> Result<(), AssetError> {
    authorize_doc(principal, ws, id, Action::Write)?;
    // Ownership: only the owner shares their doc (a non-owner with a wildcard write cap still
    // cannot re-share someone else's doc). Resolve the doc within the workspace wall first.
    let doc = store_get_doc(store, ws, id)
        .await?
        .ok_or(AssetError::NotFound)?;
    if doc.owner != principal.sub() {
        return Err(AssetError::Denied);
    }
    relate(store, ws, SHARE, id, team).await?;
    Ok(())
}
