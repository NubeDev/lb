//! Gate 3 for navs — the membership/visibility resolver (nav scope, "How it fits"). Runs strictly
//! *after* the workspace + capability gates (`authorize_nav`), never before — a membership check that
//! ran first would leak existence (the S4 ordering rule). Identical in shape to `may_read_dashboard`;
//! a nav is an asset shared through the exact same S4 `share`/`member` edges.
//!
//! A principal may read nav `n` iff ANY holds:
//!   - **`workspace`** — the nav is workspace-visible (any member with the read cap);
//!   - **owner** — `principal.sub == n.owner`;
//!   - **`team` + shared** — the nav's visibility is `team` and the principal is a `member` of a team
//!     the nav is `share`d to (the shipped S4 `share`/`member` edges, reused verbatim).
//!
//! Sharing is a live relation (not a copy): revoking a `share`/`member` edge makes the nav instantly
//! unresolvable on the next call. A non-member reading a `team` nav is **denied**.

use lb_assets::list_related;
use lb_auth::Principal;
use lb_store::Store;

use super::error::NavError;
use super::model::{Nav, Visibility};

/// The S4 edge kinds, identical to the doc-/dashboard-sharing ones: a `nav -[share]-> team` edge and
/// a `team -[member]-> user` edge.
const SHARE: &str = "share";
const MEMBER: &str = "member";

/// Resolve whether `principal` may read `nav` in workspace `ws`. `Ok(())` if any path grants it, else
/// [`NavError::Denied`]. Assumes gates 1+2 already passed.
pub async fn may_read_nav(
    store: &Store,
    principal: &Principal,
    ws: &str,
    nav: &Nav,
) -> Result<(), NavError> {
    // Owner — the simplest path (and the only path for `private`).
    if principal.sub() == nav.owner {
        return Ok(());
    }

    match nav.visibility {
        // Workspace-visible: any member who passed gates 1+2 may read it.
        Visibility::Workspace => Ok(()),
        // Private: owner only (handled above) — everyone else is denied.
        Visibility::Private => Err(NavError::Denied),
        // Team-shared: the principal must be a member of a team the nav is shared to.
        Visibility::Team => {
            let teams = list_related(store, ws, SHARE, &nav.id).await?;
            for team in &teams {
                if list_related(store, ws, MEMBER, team)
                    .await?
                    .iter()
                    .any(|m| m == principal.sub())
                {
                    return Ok(());
                }
            }
            Err(NavError::Denied)
        }
    }
}
