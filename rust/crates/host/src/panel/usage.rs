//! `panel.usage(id)` — "which dashboards reference this panel" (library-panels scope: the delete-safety
//! + "where is this used" read, and the editor's "used on N dashboards" banner). Scans the workspace's
//! dashboards for ref cells whose `panel_ref` points at `panel:{id}`, returning one [`PanelUsageRow`]
//! per referencing dashboard (id/title/cell-count).
//!
//! Gate 3 applies: only dashboards the caller may **read** are counted (a caller never learns a
//! dashboard exists that they cannot see — rule 5/6). Consistent with `dashboard.list`; `force`-delete
//! tolerates references in dashboards outside the caller's view (they degrade to the placeholder).

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_panel;
use super::error::PanelError;
use super::model::PanelUsageRow;
use crate::dashboard::{may_read_dashboard, scan_dashboards};

/// Normalize a panel id to its canonical `panel:{id}` ref form (accepts either a bare slug or the
/// full ref; the ref cell stores the full form).
fn canonical_ref(id: &str) -> String {
    if id.starts_with("panel:") {
        id.to_string()
    } else {
        format!("panel:{id}")
    }
}

/// List the dashboards in `ws` (readable by `principal`) that reference panel `id`. The `panel.usage`
/// verb — gates on `mcp:panel.usage:call`, then defers to [`scan_usage`].
pub async fn panel_usage(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Vec<PanelUsageRow>, PanelError> {
    authorize_panel(principal, ws, "panel.usage")?;
    scan_usage(store, principal, ws, id).await
}

/// The cap-free usage scan shared by `panel.usage` (after its gate) and `panel.delete`'s delete-safety
/// pre-check (already gated on `panel.delete`) — so delete never demands the `panel.usage` cap.
pub(super) async fn scan_usage(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Vec<PanelUsageRow>, PanelError> {
    let want = canonical_ref(id);
    let bare = want.trim_start_matches("panel:").to_string();

    let dashboards = scan_dashboards(store, ws).await?;
    let mut out = Vec::new();
    for d in &dashboards {
        if d.deleted {
            continue;
        }
        let cells = d
            .cells
            .iter()
            .filter(|c| c.panel_ref == want || c.panel_ref == bare)
            .count();
        if cells == 0 {
            continue;
        }
        // Gate 3 — only surface dashboards the caller may read (no existence leak).
        if may_read_dashboard(store, principal, ws, d).await.is_err() {
            continue;
        }
        out.push(PanelUsageRow {
            dashboard: d.id.clone(),
            title: d.title.clone(),
            cells,
        });
    }
    Ok(out)
}
