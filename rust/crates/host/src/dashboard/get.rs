//! `dashboard.get(id)` — the three-gate read verb (dashboard scope, "MCP surface"). Gates run in
//! exact order: 1+2 (`authorize_dashboard`) before any fetch (no existence signal to an outsider),
//! then fetch, then gate 3 (`may_read_dashboard`) — a non-member of a team-shared dashboard is
//! denied. A tombstoned dashboard reads as `NotFound`.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::model::Dashboard;
use super::store::read_dashboard;
use super::visibility::may_read_dashboard;

/// Read dashboard `id` in `ws` for `principal`, if all three gates pass.
pub async fn dashboard_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Dashboard, DashboardError> {
    // Gates 1 + 2: workspace isolation, then the read capability — before any fetch.
    authorize_dashboard(principal, ws, "dashboard.get")?;

    let dashboard = read_dashboard(store, ws, id)
        .await?
        .filter(|d| !d.deleted)
        .ok_or(DashboardError::NotFound)?;

    // Gate 3: membership/visibility. Denied otherwise (the non-member deny).
    may_read_dashboard(store, principal, ws, &dashboard).await?;
    Ok(dashboard)
}
