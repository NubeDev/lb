//! `panel.share(id, {visibility, team?})` — set a panel private/team/workspace (library-panels scope,
//! "Share"). For the `team` case it writes the **shipped S4 `share` edge** (`panel -[share]-> team`),
//! so the existing gate-3 read check (`may_read_panel`) applies unchanged. Idempotent. Owner-only.
//! Gated `mcp:panel.share:call`.
//!
//! **Sharing shares the definition, never data access** — a panel's `sources[]` still re-check under
//! the viewer's caps per call (the "sharing never widens" thesis; enforced at the render path, tested
//! at the gateway).

use lb_assets::relate;
use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_panel;
use super::error::PanelError;
use super::model::{Panel, Visibility};
use super::store::{read_panel, write_panel};

/// The S4 share edge kind — `panel -[share]-> team`.
const SHARE: &str = "share";

/// Set `id`'s visibility in `ws` as `principal` (owner-only), at logical time `now`. When `team` is
/// given (the `team` tier), writes the `share` edge so members of that team can read it. Returns the
/// updated record.
pub async fn panel_share(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    visibility: Visibility,
    team: Option<&str>,
    now: u64,
) -> Result<Panel, PanelError> {
    authorize_panel(principal, ws, "panel.share")?;

    let mut panel = read_panel(store, ws, id)
        .await?
        .filter(|p| !p.deleted)
        .ok_or(PanelError::NotFound)?;

    // Only the owner shares their panel (mirrors `dashboard.share`).
    if panel.owner != principal.sub() {
        return Err(PanelError::Denied);
    }

    if let Some(team) = team {
        if team.is_empty() {
            return Err(PanelError::BadInput("empty team".into()));
        }
        relate(store, ws, SHARE, id, team).await?;
    } else if visibility == Visibility::Team {
        return Err(PanelError::BadInput(
            "team visibility requires a team".into(),
        ));
    }

    panel.visibility = visibility;
    panel.updated_ts = now;
    write_panel(store, ws, &panel).await?;
    Ok(panel)
}
