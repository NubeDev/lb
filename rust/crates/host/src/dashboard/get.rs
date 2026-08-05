//! `dashboard.get(id)` — the four-gate read verb (dashboard scope, "MCP surface"). Gates run in
//! exact order: 1+2 (`authorize_dashboard`) before any fetch (no existence signal to an outsider),
//! then gate 2b — **record reach** (`dashboard_reach_ok`, nav-reach-record scope), also pre-fetch
//! since it needs only the id — then fetch, then gate 3 (`may_read_dashboard`) — a non-member of a
//! team-shared dashboard is denied. A tombstoned dashboard reads as `NotFound`.
//!
//! Gate 2b is a pure NARROWING: it is unarmed (always true) unless the caller's token carries
//! record-granular reach derived from a curated nav, so it can only ever subtract from what gate 3
//! would allow. It runs before the fetch for the same reason gates 1+2 do — a subject whose menu does
//! not name a board should learn nothing about whether it exists.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::model::Dashboard;
use super::store::read_dashboard;
use super::visibility::may_read_dashboard;
use crate::nav::dashboard_reach_ok;

/// Read dashboard `id` in `ws` for `principal`, if all four gates pass.
pub async fn dashboard_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Dashboard, DashboardError> {
    // Gates 1 + 2: workspace isolation, then the read capability — before any fetch.
    authorize_dashboard(principal, ws, "dashboard.get")?;

    // Gate 2b: record reach — a curated nav that names dashboards IS the boundary at record
    // granularity, so a board the caller's own menu does not name is closed even if it is
    // workspace-visible. Unarmed (open) for every non-curated token.
    if !dashboard_reach_ok(principal, ws, id) {
        return Err(DashboardError::Denied);
    }

    let mut dashboard = read_dashboard(store, ws, id)
        .await?
        .filter(|d| !d.deleted)
        .ok_or(DashboardError::NotFound)?;

    // Gate 3: membership/visibility. Denied otherwise (the non-member deny).
    may_read_dashboard(store, principal, ws, &dashboard).await?;

    // Hydrate library-panel refs host-side (library-panels scope Decision: the ONE hydration seam).
    // Each ref cell's `panel_ref` expands to a resolved v3 cell under the VIEWER's three gates — an
    // unreadable/dangling ref degrades to the placeholder, never a leaked spec. Inline cells untouched.
    dashboard.cells =
        crate::panel::hydrate_cells(store, principal, ws, std::mem::take(&mut dashboard.cells))
            .await;
    Ok(dashboard)
}
