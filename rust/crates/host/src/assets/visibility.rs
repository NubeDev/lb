//! Gate 3 for docs — the membership resolver: *which* doc, within the workspace, a principal
//! may read (files scope). This is the second isolation layer the tenancy scope deferred — it
//! runs strictly *after* the workspace + capability gates (`authorize_doc`), never before.
//!
//! A principal may read doc `d` iff ANY holds:
//!   - **owner** — `principal.sub == d.owner`;
//!   - **shared** — the principal is a `member` of a team `d` is `share`d to;
//!   - **linked** — the principal may `sub` a channel `d` is `link`ed into (reusing the channel
//!     capability gate — a doc linked into a channel is readable by that channel's audience).
//!
//! Sharing is a live relation (not a content copy), so revoking a `share`/`link`/`member` edge
//! makes the doc instantly unreadable on the next call — the relations are re-resolved here
//! every read.

use lb_assets::{list_related, Doc};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use crate::channel::authorize_channel;

use super::error::AssetError;

/// Relation kinds (files scope table). Kept here so the doc gate and the share/link verbs name
/// the same strings.
pub(crate) const SHARE: &str = "share";
pub(crate) const LINK: &str = "link";
pub(crate) const MEMBER: &str = "member";

/// Resolve whether `principal` may read `doc` in workspace `ws`. Returns `Ok(())` if any path
/// grants it, else [`AssetError::Denied`]. Assumes gates 1+2 already passed.
pub async fn may_read_doc(
    store: &Store,
    principal: &Principal,
    ws: &str,
    doc: &Doc,
) -> Result<(), AssetError> {
    // Owner — the simplest path.
    if principal.sub() == doc.owner {
        return Ok(());
    }

    // Shared — the principal is a member of any team the doc is shared to.
    let teams = list_related(store, ws, SHARE, &doc.id).await?;
    for team in &teams {
        if list_related(store, ws, MEMBER, team)
            .await?
            .iter()
            .any(|m| m == principal.sub())
        {
            return Ok(());
        }
    }

    // Linked — the principal may `sub` any channel the doc is linked into. Reuse the channel
    // capability gate verbatim (a doc in a channel inherits the channel's read audience).
    let channels = list_related(store, ws, LINK, &doc.id).await?;
    for cid in &channels {
        if authorize_channel(principal, ws, cid, Action::Sub).is_ok() {
            return Ok(());
        }
    }

    Err(AssetError::Denied)
}
