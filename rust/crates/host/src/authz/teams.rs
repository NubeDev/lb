//! `teams.create` / `teams.list` — the admin verbs over the first-class team record (authz-grants
//! scope). Gated by `mcp:teams.manage:call` (create) / `mcp:teams.list:call` (list), workspace-first.
//!
//! `teams.manage` is the dedicated admin cap that replaces the S4 doc-write stopgap for membership
//! administration — the same cap `members.add`/`members.remove` reuse (admin-crud scope). `create`
//! only records the team's existence + display name; the `member` edges are managed by the members
//! service. Idempotent on the team id.

use lb_auth::Principal;
use lb_authz::{team_create, team_list, Team};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::AuthzError;

/// Create (or rename) team `team` with display `name` in `ws`. Gated by `mcp:teams.manage:call`.
pub async fn teams_create(
    store: &Store,
    principal: &Principal,
    ws: &str,
    team: &str,
    name: &str,
) -> Result<(), AuthzError> {
    authorize_tool(principal, ws, "teams.manage").map_err(|_| AuthzError::Denied)?;
    team_create(store, ws, team, name).await?;
    Ok(())
}

/// List every team in `ws`. Gated by `mcp:teams.list:call`.
pub async fn teams_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Team>, AuthzError> {
    authorize_tool(principal, ws, "teams.list").map_err(|_| AuthzError::Denied)?;
    Ok(team_list(store, ws).await?)
}
