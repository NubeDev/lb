//! `dashboard.share(id, {visibility, team?})` — set a dashboard private/team/workspace (dashboard
//! scope, "Share"). For the `team` case it writes the **shipped S4 `share` edge** (`dashboard
//! -[share]-> team`), so the existing gate-3 read check (`may_read_dashboard`) applies unchanged.
//! Idempotent (re-share = same edge upsert + same visibility). Gated `mcp:dashboard.share`.
//!
//! Owner-only UNLESS the caller also holds `mcp:dashboard.share_any:call` (an admin-granted cap,
//! checked second so a non-admin never pays its cost) — an admin who may delete and fix a board they
//! do not own must also be able to RE-SCOPE its visibility, or the override story has a hole the
//! first time a board is shared too widely (ext-managed-dashboards D2: the triad
//! `delete_any`/`save_any`/`share_any` ships together). Its own capability, never an ambient
//! admin-role test.

use lb_assets::relate;
use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::model::{Dashboard, Visibility};
use super::store::{read_dashboard, write_dashboard};

/// The S4 share edge kind (identical to doc sharing) — `dashboard -[share]-> team`.
const SHARE: &str = "share";

/// The `dashboard.share` descriptor — a real arg schema so a model advertised the verb can form
/// the call (live, the agent's share failed on a mis-typed `now` it had to guess).
pub fn share_descriptor() -> lb_mcp::ToolDescriptor {
    lb_mcp::ToolDescriptor {
        emits_external: false,
        name: "dashboard.share".to_string(),
        title: "Set a dashboard's visibility (private / team / workspace)".to_string(),
        group: "dashboard".to_string(),
        input_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "x-lb": { "label": "Dashboard id" } },
                "visibility": { "type": "string", "enum": ["private", "team", "workspace"], "x-lb": { "label": "Visibility" } },
                "team": { "type": "string", "x-lb": { "label": "Team", "description": "Required only for visibility=team" } },
                "now": { "type": "integer", "x-lb": { "label": "Timestamp", "description": "Logical time of the share — unix epoch seconds" } }
            },
            "required": ["id", "visibility", "now"]
        })),
        result: None,
    }
}

/// Set `id`'s visibility in `ws` as `principal` (owner-only, or a non-owner also holding
/// `mcp:dashboard.share_any:call`), at logical time `now`. When `team` is
/// given (the `team` tier), writes the `share` edge so members of that team can read it. Returns the
/// updated record.
pub async fn dashboard_share(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    visibility: Visibility,
    team: Option<&str>,
    now: u64,
) -> Result<Dashboard, DashboardError> {
    authorize_dashboard(principal, ws, "dashboard.share")?;

    let mut dashboard = read_dashboard(store, ws, id)
        .await?
        .filter(|d| !d.deleted)
        .ok_or(DashboardError::NotFound)?;

    // Only the owner shares their dashboard (mirrors `share_doc`) — unless the caller holds the
    // admin override. Owner check FIRST, the override attempted strictly second (`&&`
    // short-circuits), exactly as `dashboard.delete` treats `delete_any`.
    if dashboard.owner != principal.owner_sub()
        && authorize_dashboard(principal, ws, "dashboard.share_any").is_err()
    {
        return Err(DashboardError::Denied);
    }

    // Writing the share edge to a team (idempotent). Required for the `team` tier; harmless for the
    // others (the edge is only consulted when visibility is `team`).
    if let Some(team) = team {
        if team.is_empty() {
            return Err(DashboardError::BadInput("empty team".into()));
        }
        relate(store, ws, SHARE, id, team).await?;
    } else if visibility == Visibility::Team {
        return Err(DashboardError::BadInput(
            "team visibility requires a team".into(),
        ));
    }

    dashboard.visibility = visibility;
    dashboard.updated_ts = now;
    write_dashboard(store, ws, &dashboard).await?;
    Ok(dashboard)
}
