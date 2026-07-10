//! `report.share(id, {visibility, team?})` — set a report private/team/workspace (reports scope).
//! For the `team` case it writes the shipped S4 `share` edge (`report -[share]-> team`), so the
//! existing gate-3 read check (`may_read_report`) applies unchanged. Idempotent. Owner-only. Gated
//! `mcp:report.share:call`.
//!
//! **Sharing shares the definition, never data access** — a report's embedded panels re-check under
//! the viewer's caps at render (the "sharing never widens" thesis; the PDF is different by nature —
//! it embeds pixels under the exporter's caps).

use lb_assets::relate;
use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_report;
use super::error::ReportError;
use super::model::{Report, Visibility};
use super::store::{read_report, write_report};

/// The S4 share edge kind — `report -[share]-> team`.
const SHARE: &str = "share";

/// Set `id`'s visibility in `ws` as `principal` (owner-only), at logical time `now`. When `team` is
/// given (the `team` tier), writes the `share` edge so members of that team can read it. Returns the
/// updated record.
pub async fn report_share(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    visibility: Visibility,
    team: Option<&str>,
    now: u64,
) -> Result<Report, ReportError> {
    authorize_report(principal, ws, "report.share")?;

    let mut report = read_report(store, ws, id)
        .await?
        .filter(|r| !r.deleted)
        .ok_or(ReportError::NotFound)?;

    if report.owner != principal.owner_sub() {
        return Err(ReportError::Denied);
    }

    if let Some(team) = team {
        if team.is_empty() {
            return Err(ReportError::BadInput("empty team".into()));
        }
        relate(store, ws, SHARE, id, team).await?;
    } else if visibility == Visibility::Team {
        return Err(ReportError::BadInput(
            "team visibility requires a team".into(),
        ));
    }

    report.visibility = visibility;
    report.updated_ts = now;
    write_report(store, ws, &report).await?;
    Ok(report)
}
