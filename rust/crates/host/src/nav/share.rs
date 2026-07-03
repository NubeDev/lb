//! `nav.share(id, {visibility, team?})` — set a nav private/team/workspace (nav scope, "Share"). For
//! the `team` case it writes the **shipped S4 `share` edge** (`nav -[share]-> team`), so the existing
//! gate-3 read check (`may_read_nav`) applies unchanged. Idempotent (re-share = same edge upsert +
//! same visibility). Owner-only. Gated `mcp:nav.share:call`.
//!
//! This is the ONLY authority the nav attaches to teams — and it grants NOTHING: sharing a nav to a
//! team makes the MENU visible to members, never a page. Access to a page is still decided by the
//! member's own caps + the target's visibility (nav scope, "the lens grants nothing").

use lb_assets::relate;
use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_nav;
use super::error::NavError;
use super::model::{Nav, Visibility};
use super::store::{read_nav, write_nav};

/// The S4 share edge kind (identical to dashboard/doc sharing) — `nav -[share]-> team`.
const SHARE: &str = "share";

/// Set `id`'s visibility in `ws` as `principal` (owner-only), at logical time `now`. When `team` is
/// given (the `team` tier), writes the `share` edge so members of that team can resolve it. Returns
/// the updated record.
pub async fn nav_share(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    visibility: Visibility,
    team: Option<&str>,
    now: u64,
) -> Result<Nav, NavError> {
    authorize_nav(principal, ws, "nav.share")?;

    let mut nav = read_nav(store, ws, id)
        .await?
        .filter(|n| !n.deleted)
        .ok_or(NavError::NotFound)?;

    if nav.owner != principal.sub() {
        return Err(NavError::Denied);
    }

    if let Some(team) = team {
        if team.is_empty() {
            return Err(NavError::BadInput("empty team".into()));
        }
        relate(store, ws, SHARE, id, team).await?;
    } else if visibility == Visibility::Team {
        return Err(NavError::BadInput("team visibility requires a team".into()));
    }

    nav.visibility = visibility;
    nav.updated_ts = now;
    write_nav(store, ws, &nav).await?;
    Ok(nav)
}
