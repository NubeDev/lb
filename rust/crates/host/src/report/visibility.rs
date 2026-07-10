//! Gate 3 for reports — the membership/visibility resolver (reports scope, "Capabilities"). Runs
//! strictly *after* the workspace + capability gates (`authorize_report`), never before — a
//! membership check that ran first would leak existence (the S4 ordering rule). Identical logic to
//! `panel::visibility::may_read_panel`, over the same shipped S4 `share`/`member` edges.
//!
//! A principal may read report `r` iff ANY holds: `workspace` visibility; owner; or `team`
//! visibility + the principal is a `member` of a team the report is `share`d to.

use lb_assets::list_related;
use lb_auth::Principal;
use lb_store::Store;

use super::error::ReportError;
use super::model::{Report, Visibility};

const SHARE: &str = "share";
const MEMBER: &str = "member";

/// Resolve whether `principal` may read `report` in workspace `ws`. Returns `Ok(())` if any path
/// grants it, else [`ReportError::Denied`]. Assumes gates 1+2 already passed.
pub async fn may_read_report(
    store: &Store,
    principal: &Principal,
    ws: &str,
    report: &Report,
) -> Result<(), ReportError> {
    if principal.owner_sub() == report.owner {
        return Ok(());
    }
    match report.visibility {
        Visibility::Workspace => Ok(()),
        Visibility::Private => Err(ReportError::Denied),
        Visibility::Team => {
            let teams = list_related(store, ws, SHARE, &report.id).await?;
            for team in &teams {
                if list_related(store, ws, MEMBER, team)
                    .await?
                    .iter()
                    .any(|m| m == principal.owner_sub())
                {
                    return Ok(());
                }
            }
            Err(ReportError::Denied)
        }
    }
}
