//! `dashboard.list()` — the roster verb (dashboard scope, "Get / list"). Returns exactly the
//! dashboards the caller can reach (own + team-shared + workspace-visible), as cheap summaries
//! (id/title/visibility/updated_ts, **no cell bodies**). Gates 1+2 first, then gate-3 filters the
//! scanned set row-by-row — so a non-member never even sees a team-shared dashboard's title.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::model::DashboardSummary;
use super::store::scan_dashboards;
use super::visibility::may_read_dashboard;

/// List the dashboards in `ws` that `principal` may read. Tombstoned dashboards are excluded.
pub async fn dashboard_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<DashboardSummary>, DashboardError> {
    authorize_dashboard(principal, ws, "dashboard.list")?;

    let all = scan_dashboards(store, ws).await?;
    let mut out = Vec::new();
    for d in &all {
        if d.deleted {
            continue;
        }
        // Gate 3 per row — the roster shows only what the caller may read (membership-filtered).
        if may_read_dashboard(store, principal, ws, d).await.is_ok() {
            out.push(DashboardSummary::from(d));
        }
    }
    Ok(out)
}
