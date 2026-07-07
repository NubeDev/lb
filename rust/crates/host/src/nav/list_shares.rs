//! `nav.list_shares(id)` — enumerate the teams a nav is currently `share`d to (nav scope, the read
//! the builder needs to render its share roster). Returns the live `share` edge targets via the
//! shipped S4 `list_related` (`lb-assets`) — the exact set gate-3 (`may_read_nav`) walks, so what
//! the builder shows is what the resolver sees. Tombstoned edges are skipped by `list_related`.
//!
//! Owner-only read (a non-owner has no business editing the share roster); a member who can READ a
//! team-shared nav already sees the nav in their `nav.list` — exposing its other team shares to a
//! peer editor would leak which other teams exist. Gated `mcp:nav.share:call` (the same write cap
//! the builder already requires for `nav.share`/`nav.unshare`), so the cap surface stays one verb.

use lb_assets::list_related;
use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_nav;
use super::error::NavError;
use super::store::read_nav;

/// The S4 share edge kind (identical to dashboard/doc sharing) — `nav -[share]-> team`.
const SHARE: &str = "share";

/// List the live `team:*` subjects nav `id` is shared to in workspace `ws`, as the nav's owner.
pub async fn nav_list_shares(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Vec<String>, NavError> {
    authorize_nav(principal, ws, "nav.share")?;

    let nav = read_nav(store, ws, id)
        .await?
        .filter(|n| !n.deleted)
        .ok_or(NavError::NotFound)?;

    if nav.owner != principal.sub() {
        return Err(NavError::Denied);
    }

    Ok(list_related(store, ws, SHARE, id).await?)
}
