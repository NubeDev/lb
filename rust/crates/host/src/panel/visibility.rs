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

use crate::teams::bare_team;

use super::error::PanelError;
use super::model::{Panel, Visibility};

/// The S4 edge kinds, identical to the doc/dashboard sharing ones.
const SHARE: &str = "share";
const MEMBER: &str = "member";

/// Is `sub` a member of the team named by `subject` (a `share` edge's `b`, so either the bare `ops` or
/// the prefixed `team:ops` form)?
///
/// A team has TWO identities: the membership graph keys the `member` edge on the **bare** id, while
/// the grant store uses `Subject::Team("ops").as_key()` → `team:ops`
/// (`docs/debugging/dashboard/share-closure-team-prefix-mismatch.md`). Both shapes exist in stores
/// written before that was settled, so probe the bare name FIRST — the one the live system uses — and
/// tolerate the prefixed form rather than assume a single shape. This mirrors what
/// `access_check::gate3_identity` already does, and is why a panel and a dashboard shared to the same
/// team now resolve alike: `may_read_dashboard` normalised to bare while this resolver looked the raw
/// subject up verbatim, so one of the pair was always wrong whichever convention a caller wrote.
async fn team_has_member(
    store: &Store,
    ws: &str,
    subject: &str,
    sub: &str,
) -> Result<bool, lb_store::StoreError> {
    for key in [bare_team(subject), subject] {
        if list_related(store, ws, MEMBER, key)
            .await?
            .iter()
            .any(|m| m == sub)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

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
                if team_has_member(store, ws, team, principal.owner_sub()).await? {
                    return Ok(());
                }
            }
            Err(PanelError::Denied)
        }
    }
}
