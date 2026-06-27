//! `teams.rename` — update a team's display name (admin-crud scope). Gated by `mcp:teams.manage:call`,
//! workspace-first. Reuses the idempotent `team_create` upsert (create-or-update-name is one verb at
//! the store; the host names the intent). Membership edges are untouched.

use lb_auth::Principal;
use lb_authz::team_create;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::TeamsError;

/// Rename `team` to `name` in workspace `ws` as `principal`. Idempotent.
pub async fn teams_rename(
    store: &Store,
    principal: &Principal,
    ws: &str,
    team: &str,
    name: &str,
) -> Result<(), TeamsError> {
    authorize_tool(principal, ws, "teams.manage").map_err(|_| TeamsError::Denied)?;
    team_create(store, ws, team, name).await?;
    Ok(())
}
