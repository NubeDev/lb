//! `report.list()` — the roster verb (reports scope, "MCP surface"). Returns exactly the reports the
//! caller can reach (own + team-shared + workspace-visible), as cheap summaries (id/title/visibility/
//! updated_ts/block_count, **no block bodies**). Gates 1+2 first, then gate-3 filters the scanned set
//! row-by-row — so a non-member never even sees a team-shared report's title.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_report;
use super::error::ReportError;
use super::model::ReportSummary;
use super::store::scan_reports;
use super::visibility::may_read_report;

/// List the reports in `ws` that `principal` may read. Tombstoned reports are excluded.
pub async fn report_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<ReportSummary>, ReportError> {
    authorize_report(principal, ws, "report.list")?;

    let all = scan_reports(store, ws).await?;
    let mut out = Vec::new();
    for r in &all {
        if r.deleted {
            continue;
        }
        if may_read_report(store, principal, ws, r).await.is_ok() {
            out.push(ReportSummary::from(r));
        }
    }
    Ok(out)
}
