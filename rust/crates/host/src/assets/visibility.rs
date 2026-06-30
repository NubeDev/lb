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

use lb_assets::{list_related, Asset, Doc};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use crate::channel::authorize_channel;

use super::error::AssetError;

/// Relation kinds (files + document-store scopes). Kept here so the doc/asset gates and the
/// share/link/embed verbs name the same strings.
pub(crate) const SHARE: &str = "share";
pub(crate) const LINK: &str = "link";
pub(crate) const MEMBER: &str = "member";
/// A doc→doc internal link edge (document-store scope). Kept distinct from `LINK`
/// (doc→channel, shipped S4) so the two never collide in the `(kind, a, b)` table.
pub(crate) const DOCLINK: &str = "doclink";
/// A doc→asset embed edge (document-store scope): doc `a` embeds asset `b`.
pub(crate) const EMBED: &str = "embed";

/// A subject prefix for a share-to-an-individual (document-store scope: the `user` subject).
pub(crate) const USER_PREFIX: &str = "user:";

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

    // Shared — the principal is a member of any team the doc is shared to, OR is the individual
    // the doc is shared to (a `user:…` subject, document-store scope: the `user` subject). The
    // same `share` edge backs both: `b` distinguishes `team:…` (resolve membership) from
    // `user:…` (a direct match on the full principal sub, e.g. `user:ben`).
    let subjects = list_related(store, ws, SHARE, &doc.id).await?;
    for subject in &subjects {
        if subject.starts_with(USER_PREFIX) {
            // A direct individual share — the subject IS the principal's full sub (`user:…`).
            if subject == &principal.sub() {
                return Ok(());
            }
            continue;
        }
        // Otherwise treat `subject` as a team and check membership.
        if list_related(store, ws, MEMBER, subject)
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

/// Resolve whether `principal` may read `asset` in workspace `ws`. The asset analog of
/// [`may_read_doc`] (document-store scope). A reader reaches a binary asset if ANY holds:
///   - **owner** — `principal.sub == asset.owner`;
///   - **shared** — the asset is shared to a team the principal is a member of, or to the
///     principal directly (`user:…`);
///   - **embedded** — the asset is embedded by a doc the principal may read (`embed` edge),
///     resolved through the full doc gate so an embed never widens access (the load-bearing
///     "link/embed never widens" deny test). The embedding doc is re-gated here, not trusted.
pub async fn may_read_asset(
    store: &Store,
    principal: &Principal,
    ws: &str,
    asset: &Asset,
) -> Result<(), AssetError> {
    // Owner.
    if principal.sub() == asset.owner {
        return Ok(());
    }

    // Shared (team membership or direct user share on the full principal sub).
    let subjects = list_related(store, ws, SHARE, &asset.id).await?;
    for subject in &subjects {
        if subject.starts_with(USER_PREFIX) {
            if subject == &principal.sub() {
                return Ok(());
            }
            continue;
        }
        if list_related(store, ws, MEMBER, subject)
            .await?
            .iter()
            .any(|m| m == principal.sub())
        {
            return Ok(());
        }
    }

    // Embedded — re-gate every doc that embeds this asset through the FULL doc gate.
    let embedders = lb_assets::list_related_inverse(store, ws, EMBED, &asset.id).await?;
    for doc_id in &embedders {
        if let Some(doc) = lb_assets::get_doc(store, ws, doc_id).await? {
            if may_read_doc(store, principal, ws, &doc).await.is_ok() {
                return Ok(());
            }
        }
    }

    Err(AssetError::Denied)
}
