//! `nav.list()` — the roster verb (nav scope, "Get / list"). Returns exactly the navs the caller can
//! reach (own + team-shared + workspace-visible), as cheap summaries (id/title/visibility/updated_ts,
//! **no `items[]` bodies**). Gates 1+2 first, then gate-3 filters the scanned set row-by-row — so a
//! non-member never even sees a team-shared nav's title.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_nav;
use super::error::NavError;
use super::model::NavSummary;
use super::store::scan_navs;
use super::visibility::may_read_nav;

/// List the navs in `ws` that `principal` may read. Tombstoned navs are excluded.
pub async fn nav_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<NavSummary>, NavError> {
    authorize_nav(principal, ws, "nav.list")?;

    let all = scan_navs(store, ws).await?;
    let mut out = Vec::new();
    for n in &all {
        if n.deleted {
            continue;
        }
        if may_read_nav(store, principal, ws, n).await.is_ok() {
            out.push(NavSummary::from(n));
        }
    }
    Ok(out)
}
