//! `report.get(id)` — the three-gate read verb (reports scope, "MCP surface"). Gates run in exact
//! order: 1+2 (`authorize_report`) before any fetch, then fetch, then gate 3 (`may_read_report`).
//! A tombstoned report reads as `NotFound`.
//!
//! After the read, **panel blocks hydrate**: each `panel` block's embedded cell is resolved through
//! the shipped `lb_host::hydrate_cells` (expands `panel_ref` under the viewer's gates, degrades a
//! missing ref to the placeholder — never fails the read), exactly like `dashboard.get`.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_report;
use super::error::ReportError;
use super::model::Report;
use super::store::read_report;
use super::visibility::may_read_report;

/// Read report `id` in `ws` for `principal`, if all three gates pass, with panel blocks hydrated.
pub async fn report_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Report, ReportError> {
    // Gates 1 + 2 before any fetch.
    authorize_report(principal, ws, "report.get")?;

    let mut report = read_report(store, ws, id)
        .await?
        .filter(|r| !r.deleted)
        .ok_or(ReportError::NotFound)?;

    // Gate 3: membership/visibility.
    may_read_report(store, principal, ws, &report).await?;

    // Hydrate every panel block's cell (ref → resolved v3 cell; missing → placeholder). One cell
    // per block, hydrated individually so a report's block order is preserved untouched.
    for block in &mut report.blocks {
        if block.kind == "panel" {
            let cell = std::mem::take(&mut block.cell);
            let mut hydrated = crate::hydrate_cells(store, principal, ws, vec![cell]).await;
            block.cell = hydrated.pop().unwrap_or_default();
        }
    }
    Ok(report)
}
