//! Gate 3 for panels — the membership/visibility resolver (library-panels scope, "Access"). Runs
//! strictly *after* the workspace + capability gates (`authorize_panel`), never before — a membership
//! check that ran first would leak existence (the S4 ordering rule). Identical logic to
//! `dashboard::visibility::may_read_dashboard`, over the same shipped S4 `share`/`member` edges.
//!
//! A principal may read panel `p` iff ANY holds: `workspace` visibility; owner; or `team` visibility +
//! the principal is a `member` of a team the panel is `share`d to. Sharing is a live relation — revoke
//! the edge and the panel is instantly unreadable on the next call.

use lb_assets::list_related;
use lb_auth::Principal;
use lb_store::Store;

use super::error::PanelError;
use super::model::{Panel, Visibility};

/// The S4 edge kinds, identical to the doc/dashboard sharing ones.
const SHARE: &str = "share";
const MEMBER: &str = "member";

/// Resolve whether `principal` may read `panel` in workspace `ws`. Returns `Ok(())` if any path grants
/// it, else [`PanelError::Denied`]. Assumes gates 1+2 already passed.
pub async fn may_read_panel(
    store: &Store,
    principal: &Principal,
    ws: &str,
    panel: &Panel,
) -> Result<(), PanelError> {
    if principal.owner_sub() == panel.owner {
        return Ok(());
    }
    match panel.visibility {
        Visibility::Workspace => Ok(()),
        Visibility::Private => Err(PanelError::Denied),
        Visibility::Team => {
            let teams = list_related(store, ws, SHARE, &panel.id).await?;
            for team in &teams {
                if list_related(store, ws, MEMBER, team)
                    .await?
                    .iter()
                    .any(|m| m == principal.owner_sub())
                {
                    return Ok(());
                }
            }
            Err(PanelError::Denied)
        }
    }
}
