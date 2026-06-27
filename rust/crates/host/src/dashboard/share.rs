//! `dashboard.share(id, {visibility, team?})` — set a dashboard private/team/workspace (dashboard
//! scope, "Share"). For the `team` case it writes the **shipped S4 `share` edge** (`dashboard
//! -[share]-> team`), so the existing gate-3 read check (`may_read_dashboard`) applies unchanged.
//! Idempotent (re-share = same edge upsert + same visibility). Owner-only. Gated `mcp:dashboard.share`.

use lb_assets::relate;
use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::model::{Dashboard, Visibility};
use super::store::{read_dashboard, write_dashboard};

/// The S4 share edge kind (identical to doc sharing) — `dashboard -[share]-> team`.
const SHARE: &str = "share";

/// Set `id`'s visibility in `ws` as `principal` (owner-only), at logical time `now`. When `team` is
/// given (the `team` tier), writes the `share` edge so members of that team can read it. Returns the
/// updated record.
pub async fn dashboard_share(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    visibility: Visibility,
    team: Option<&str>,
    now: u64,
) -> Result<Dashboard, DashboardError> {
    authorize_dashboard(principal, ws, "dashboard.share")?;

    let mut dashboard = read_dashboard(store, ws, id)
        .await?
        .filter(|d| !d.deleted)
        .ok_or(DashboardError::NotFound)?;

    // Only the owner shares their dashboard (mirrors `share_doc`).
    if dashboard.owner != principal.sub() {
        return Err(DashboardError::Denied);
    }

    // Writing the share edge to a team (idempotent). Required for the `team` tier; harmless for the
    // others (the edge is only consulted when visibility is `team`).
    if let Some(team) = team {
        if team.is_empty() {
            return Err(DashboardError::BadInput("empty team".into()));
        }
        relate(store, ws, SHARE, id, team).await?;
    } else if visibility == Visibility::Team {
        return Err(DashboardError::BadInput(
            "team visibility requires a team".into(),
        ));
    }

    dashboard.visibility = visibility;
    dashboard.updated_ts = now;
    write_dashboard(store, ws, &dashboard).await?;
    Ok(dashboard)
}
