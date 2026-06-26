//! List the caller's own docs in a workspace — requires `store:doc/*:read`, workspace-first.
//!
//! At S4 this lists docs the principal *owns* (the listing view of "my docs"). Docs shared *to*
//! the principal are read by id through `get_doc` (which resolves membership); a "shared with
//! me" listing is a follow-up (files scope open questions). Scoping the list to the owner keeps
//! it from leaking another user's titles within the workspace.

use lb_assets::{list_docs as store_list_docs, Doc};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_doc;
use super::error::AssetError;

/// Return the docs `principal` owns in workspace `ws`, oldest→newest. Requires the doc read
/// capability; gate 1 refuses a cross-workspace list before the query runs.
pub async fn list_docs(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Doc>, AssetError> {
    // `*` resource: a list isn't about one doc id, so authorize against the surface wildcard.
    authorize_doc(principal, ws, "*", Action::Read)?;
    Ok(store_list_docs(store, ws, principal.sub()).await?)
}
