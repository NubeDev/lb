//! `panel.list()` — the roster verb (library-panels scope, "Get / list"). Returns exactly the panels
//! the caller can reach (own + team-shared + workspace-visible), as cheap summaries
//! (id/title/view/visibility/updated_ts, **no spec bodies, no usage count**). Gates 1+2 first, then
//! gate-3 filters the scanned set row-by-row — so a non-member never even sees a team-shared panel's
//! title.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_panel;
use super::error::PanelError;
use super::model::PanelSummary;
use super::store::scan_panels;
use super::visibility::may_read_panel;

/// List the panels in `ws` that `principal` may read. Tombstoned panels are excluded.
pub async fn panel_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<PanelSummary>, PanelError> {
    authorize_panel(principal, ws, "panel.list")?;

    let all = scan_panels(store, ws).await?;
    let mut out = Vec::new();
    for p in &all {
        if p.deleted {
            continue;
        }
        // Gate 3 per row — the roster shows only what the caller may read (membership-filtered).
        if may_read_panel(store, principal, ws, p).await.is_ok() {
            out.push(PanelSummary::from(p));
        }
    }
    Ok(out)
}
