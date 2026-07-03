//! `nav.get(id)` — the three-gate read verb (nav scope, "MCP surface"). Gates run in exact order:
//! 1+2 (`authorize_nav`) before any fetch (no existence signal to an outsider), then fetch, then
//! gate 3 (`may_read_nav`) — a non-member of a team-shared nav is denied. A tombstoned nav reads as
//! `NotFound`. Returns the FULL record (`items[]` bodies) — the builder loads this to edit.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_nav;
use super::error::NavError;
use super::model::Nav;
use super::store::read_nav;
use super::visibility::may_read_nav;

/// Read nav `id` in `ws` for `principal`, if all three gates pass.
pub async fn nav_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Nav, NavError> {
    authorize_nav(principal, ws, "nav.get")?;

    let nav = read_nav(store, ws, id)
        .await?
        .filter(|n| !n.deleted)
        .ok_or(NavError::NotFound)?;

    may_read_nav(store, principal, ws, &nav).await?;
    Ok(nav)
}
