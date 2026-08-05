//! `dashboard.get(id)` — the four-gate read verb (dashboard scope, "MCP surface"). Gates run in
//! exact order: 1+2 (`authorize_dashboard`) before any fetch (no existence signal to an outsider),
//! then fetch, then gate 3 (`may_read_dashboard`) — a non-member of a team-shared dashboard is
//! denied — then gate 4, **record reach** (`reach_gate::reach_allows`, nav-reach-record scope). A
//! tombstoned dashboard reads as `NotFound`.
//!
//! Gate 4 is a pure NARROWING: it is unarmed (always true) unless the caller's token carries
//! record-granular reach derived from a nav they were HANDED, so it can only ever subtract from what
//! gate 3 would allow. It runs LAST, after the fetch, because it needs the record's `owner` — the
//! author of a board always reaches it, whatever their menu says (see `reach_gate` for why that valve
//! exists). Denying post-fetch costs no existence signal that gate 3 does not already give.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::model::Dashboard;
use super::reach_gate::reach_allows;
use super::store::read_dashboard;
use super::visibility::may_read_dashboard;

/// Read dashboard `id` in `ws` for `principal`, if all four gates pass.
pub async fn dashboard_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Dashboard, DashboardError> {
    // Gates 1 + 2: workspace isolation, then the read capability — before any fetch.
    authorize_dashboard(principal, ws, "dashboard.get")?;

    let mut dashboard = read_dashboard(store, ws, id)
        .await?
        .filter(|d| !d.deleted)
        .ok_or(DashboardError::NotFound)?;

    // Gate 3: membership/visibility. Denied otherwise (the non-member deny).
    may_read_dashboard(store, principal, ws, &dashboard).await?;

    // Gate 4: record reach — a nav the caller was HANDED that names dashboards is the boundary at
    // record granularity, so a board it does not name is closed even if workspace-visible. Never
    // closes a board the caller OWNS, and unarmed entirely for a fallback/self-picked/legacy token.
    if !reach_allows(principal, ws, &dashboard) {
        return Err(DashboardError::Denied);
    }

    // Hydrate library-panel refs host-side (library-panels scope Decision: the ONE hydration seam).
    // Each ref cell's `panel_ref` expands to a resolved v3 cell under the VIEWER's three gates — an
    // unreadable/dangling ref degrades to the placeholder, never a leaked spec. Inline cells untouched.
    dashboard.cells =
        crate::panel::hydrate_cells(store, principal, ws, std::mem::take(&mut dashboard.cells))
            .await;
    Ok(dashboard)
}
