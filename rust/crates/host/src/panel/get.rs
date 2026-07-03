//! `panel.get(id)` — the three-gate read verb (library-panels scope, "MCP surface"). Gates run in
//! exact order: 1+2 (`authorize_panel`) before any fetch (no existence signal to an outsider), then
//! fetch, then gate 3 (`may_read_panel`) — a non-member of a team-shared panel is denied. A tombstoned
//! panel reads as `NotFound`.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_panel;
use super::error::PanelError;
use super::model::Panel;
use super::store::read_panel;
use super::visibility::may_read_panel;

/// Read panel `id` in `ws` for `principal`, if all three gates pass.
pub async fn panel_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Panel, PanelError> {
    // Gates 1 + 2: workspace isolation, then the read capability — before any fetch.
    authorize_panel(principal, ws, "panel.get")?;

    let panel = read_panel(store, ws, id)
        .await?
        .filter(|p| !p.deleted)
        .ok_or(PanelError::NotFound)?;

    // Gate 3: membership/visibility. Denied otherwise (the non-member deny).
    may_read_panel(store, principal, ws, &panel).await?;
    Ok(panel)
}
