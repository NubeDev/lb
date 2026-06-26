//! Read a doc asset — the three-gate read verb, the heart of the S4 sharing story.
//!
//! Gates run in exact order (a membership check that ran before the workspace check would be a
//! leak — files scope risk):
//!   1. workspace + 2. capability — `authorize_doc(..., Read)` (`store:doc/{id}:read`);
//!   3. membership — `may_read_doc` (owner / shared-team-member / linked-channel-sub-grantee).
//!
//! A caller failing gate 1 or 2 gets `Denied` *before the doc is even fetched* (no existence
//! signal). A caller passing 1+2 but failing gate 3 also gets `Denied` — the mandatory
//! non-member deny (the S4 exit gate: "a non-member is DENIED").

use lb_assets::{get_doc as store_get_doc, Doc};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_doc;
use super::error::AssetError;
use super::visibility::may_read_doc;

/// Read doc `id` in workspace `ws` for `principal`, if all three gates pass. `NotFound` only
/// reaches a caller who passed the capability gate (so it leaks nothing to an outsider).
pub async fn get_doc(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Doc, AssetError> {
    // Gates 1 + 2: workspace isolation, then the doc read capability — before any fetch.
    authorize_doc(principal, ws, id, Action::Read)?;

    let doc = store_get_doc(store, ws, id)
        .await?
        .ok_or(AssetError::NotFound)?;

    // Gate 3: membership — owner / shared team / linked channel. Denied otherwise.
    may_read_doc(store, principal, ws, &doc).await?;
    Ok(doc)
}
