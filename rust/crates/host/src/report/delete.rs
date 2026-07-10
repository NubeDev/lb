//! `report.delete(id)` — tombstone-upsert (reports scope; §6.8 idempotent). Owner-only, like update.
//! Gated by `mcp:report.delete:call`. **Plain soft-delete** — `report.usage` is deferred (scope
//! Decision 4: nothing references a report yet), so there is no in-use check. A re-delete or a
//! delete of an absent report is an idempotent no-op.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_report;
use super::error::ReportError;
use super::store::{read_report, write_report};

/// Soft-delete report `id` in `ws` as `principal`, at logical time `now`. Idempotent: an absent or
/// already-tombstoned report is a no-op. Only the owner may delete.
pub async fn report_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<(), ReportError> {
    authorize_report(principal, ws, "report.delete")?;

    match read_report(store, ws, id).await? {
        None => Ok(()),
        Some(r) if r.deleted => Ok(()),
        Some(mut r) => {
            if r.owner != principal.owner_sub() {
                return Err(ReportError::Denied);
            }
            r.deleted = true;
            r.updated_ts = now;
            write_report(store, ws, &r).await?;
            Ok(())
        }
    }
}
