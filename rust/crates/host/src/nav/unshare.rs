//! `nav.unshare(id, team)` — revoke one of a nav's `share` edges (nav scope, the missing mirror of
//! `nav.share`). Calls the shipped S4 `unrelate` (`lb-assets`) so the next `nav.resolve` /
//! `nav.list` / gate-3 read instantly stops seeing that team — sharing is a *live relation*, never a
//! content copy. Owner-only, gated `mcp:nav.share:call` (the same cap as `nav.share`: it is the
//! inverse write, no separate grant). Idempotent (revoking a never-shared edge is a no-op tombstone).
//!
//! Only the **team** axis is revocable here — a nav has no per-user share by design (the visibility
//! model is `private | team | workspace`); a user reaches a nav by being a `member` of a shared
//! team, managed via `teams.add_member` / `teams.remove_member`. Revoking the last team share leaves
//! the nav `visibility:team` but edgeless — the resolver then denies everyone but the owner, exactly
//! like a dashboard with no live `share` edges. The owner can flip the tier via `nav.share` if they
//! want a different default reach.

use lb_assets::unrelate;
use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_nav;
use super::error::NavError;
use super::model::Nav;
use super::store::{read_nav, write_nav};

/// The S4 share edge kind (identical to dashboard/doc sharing) — `nav -[share]-> team`.
const SHARE: &str = "share";

/// Revoke nav `id`'s share to `team` in workspace `ws`, as the nav's owner. Idempotent. Returns the
/// unchanged nav record (so the UI can refresh without a second fetch).
pub async fn nav_unshare(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    team: &str,
    now: u64,
) -> Result<Nav, NavError> {
    authorize_nav(principal, ws, "nav.share")?;

    if team.is_empty() {
        return Err(NavError::BadInput("empty team".into()));
    }

    let mut nav = read_nav(store, ws, id)
        .await?
        .filter(|n| !n.deleted)
        .ok_or(NavError::NotFound)?;

    if nav.owner != principal.sub() {
        return Err(NavError::Denied);
    }

    unrelate(store, ws, SHARE, id, team).await?;

    // Bump `updated_ts` so an LWW peer observes the share revoke (state is append-style, §6.8). The
    // record body is otherwise unchanged.
    nav.updated_ts = now;
    write_nav(store, ws, &nav).await?;
    Ok(nav)
}
