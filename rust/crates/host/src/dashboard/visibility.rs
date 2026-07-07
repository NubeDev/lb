//! Gate 3 for dashboards — the membership/visibility resolver (dashboard scope, "Access &
//! authorization"). Runs strictly *after* the workspace + capability gates (`authorize_dashboard`),
//! never before — a membership check that ran first would leak existence (the S4 ordering rule).
//!
//! A principal may read dashboard `d` iff ANY holds:
//!   - **`workspace`** — the dashboard is workspace-visible (any member with the read cap);
//!   - **owner** — `principal.sub == d.owner`;
//!   - **`team` + shared** — the dashboard's visibility is `team` and the principal is a `member` of
//!     a team the dashboard is `share`d to (the shipped S4 `share`/`member` edges, reused verbatim).
//!
//! Sharing is a live relation (not a copy): revoking a `share`/`member` edge makes the dashboard
//! instantly unreadable on the next call — the relations are re-resolved here every read. A
//! non-member reading a `team` dashboard is **denied** (the mandatory gate-3 deny, extended from S4).

use lb_assets::list_related;
use lb_auth::Principal;
use lb_store::Store;

use super::error::DashboardError;
use super::model::{Dashboard, Visibility};

/// The S4 edge kinds, identical to the doc-sharing ones (`crates/host/src/assets/visibility.rs`):
/// a `dashboard -[share]-> team` edge and a `team -[member]-> user` edge.
const SHARE: &str = "share";
const MEMBER: &str = "member";

/// Resolve whether `principal` may read `dashboard` in workspace `ws`. Returns `Ok(())` if any path
/// grants it, else [`DashboardError::Denied`]. Assumes gates 1+2 already passed.
pub async fn may_read_dashboard(
    store: &Store,
    principal: &Principal,
    ws: &str,
    dashboard: &Dashboard,
) -> Result<(), DashboardError> {
    // Owner — the simplest path (and the only path for `private`).
    if principal.owner_sub() == dashboard.owner {
        return Ok(());
    }

    match dashboard.visibility {
        // Workspace-visible: any member who passed gates 1+2 may read it.
        Visibility::Workspace => Ok(()),
        // Private: owner only (handled above) — everyone else is denied.
        Visibility::Private => Err(DashboardError::Denied),
        // Team-shared: the principal must be a member of a team the dashboard is shared to.
        Visibility::Team => {
            let teams = list_related(store, ws, SHARE, &dashboard.id).await?;
            for team in &teams {
                if list_related(store, ws, MEMBER, team)
                    .await?
                    .iter()
                    .any(|m| m == principal.owner_sub())
                {
                    return Ok(());
                }
            }
            Err(DashboardError::Denied)
        }
    }
}
