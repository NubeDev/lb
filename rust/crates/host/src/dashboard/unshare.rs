//! `dashboard.unshare(id, team)` — revoke one of a dashboard's `share` edges (dashboard scope). The
//! missing mirror of `dashboard.share`, and the exact counterpart of `nav::nav_unshare`.
//!
//! **Why it was needed.** `dashboard.share` writes edges but nothing ever removed them: flipping a
//! board back to `private` deliberately leaves the edge in place ("harmless — the edge is only
//! consulted when visibility is `team`"). That is true right up until someone sets the board to
//! `team` again, at which point every team it was EVER shared to silently regains access. There was
//! no verb, at any layer, to take a team off a board — the only exit was deleting the board or
//! deleting the team. Found 2026-08-05 while cleaning up a share made during testing.
//!
//! Owner-only, gated on the same `mcp:dashboard.share:call` as the forward write — this is its
//! inverse, not a new privilege. Idempotent: revoking an edge that was never there is a no-op.
//! Sharing is a LIVE relation, so the next gate-3 read stops seeing the team immediately.

use lb_assets::unrelate;
use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::model::Dashboard;
use super::store::{read_dashboard, write_dashboard};

/// The S4 share edge kind — `dashboard -[share]-> team`.
const SHARE: &str = "share";

/// Revoke dashboard `id`'s share to `team` in `ws`, as the owner. Idempotent. Returns the record.
pub async fn dashboard_unshare(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    team: &str,
    now: u64,
) -> Result<Dashboard, DashboardError> {
    authorize_dashboard(principal, ws, "dashboard.share")?;

    if team.is_empty() {
        return Err(DashboardError::BadInput("empty team".into()));
    }

    let mut dashboard = read_dashboard(store, ws, id)
        .await?
        .filter(|d| !d.deleted)
        .ok_or(DashboardError::NotFound)?;

    // Owner-only, with the same admin override `dashboard.share` allows (owner checked FIRST so the
    // override is only attempted for a non-owner — `&&` short-circuits).
    if dashboard.owner != principal.owner_sub()
        && authorize_dashboard(principal, ws, "dashboard.share_any").is_err()
    {
        return Err(DashboardError::Denied);
    }

    unrelate(store, ws, SHARE, id, team).await?;

    // Bump `updated_ts` so an LWW peer observes the revoke, mirroring `nav_unshare`.
    dashboard.updated_ts = now;
    write_dashboard(store, ws, &dashboard).await?;
    Ok(dashboard)
}
