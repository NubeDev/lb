//! `dashboard.list_shares(id)` — enumerate the teams a dashboard is currently `share`d to (dashboard
//! scope). The exact mirror of `nav::nav_list_shares`, reading the same S4 `share` edges via
//! `list_related` — so what a caller is shown is precisely the set gate 3 (`may_read_dashboard`)
//! walks. Tombstoned edges are skipped by `list_related`.
//!
//! **Why this read exists.** Without it there is no way to answer "can this person actually open the
//! boards in this nav?" ahead of time. The onboarding wizard's access preview was therefore guessing:
//! it treated "holds the `dashboard.list` cap" as "can read this board", which is a CAP proxy, not the
//! per-record three-gate read the resolver performs. The two disagree exactly when it matters — a
//! `private` board belonging to someone else, or a `team` board shared to a team the person is not in
//! — so the wizard cheerfully previewed a menu the user would never see (2026-08-05). A preview that
//! lies is worse than no preview.
//!
//! Owner-only, gated on `dashboard.share` — the same posture and the same cap as the nav read: a peer
//! who can merely READ a team-shared board has no business enumerating which OTHER teams hold it (that
//! leaks the existence of teams they are not in).

use lb_assets::list_related;
use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::store::read_dashboard;

/// The S4 share edge kind — `dashboard -[share]-> team`.
const SHARE: &str = "share";

/// List the live team subjects dashboard `id` is shared to in `ws`, as the dashboard's owner.
pub async fn dashboard_list_shares(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Vec<String>, DashboardError> {
    authorize_dashboard(principal, ws, "dashboard.share")?;

    let dashboard = read_dashboard(store, ws, id)
        .await?
        .filter(|d| !d.deleted)
        .ok_or(DashboardError::NotFound)?;

    if dashboard.owner != principal.owner_sub() {
        return Err(DashboardError::Denied);
    }

    Ok(list_related(store, ws, SHARE, id).await?)
}
