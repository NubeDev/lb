//! `list_members` — every user in a team, for the members/teams UI.
//!
//! Gated by `mcp:members.list:call` (workspace-first). Reads the live `member` edges via
//! `lb_assets::list_related`, so a revoked membership disappears immediately (the edges are
//! re-resolved every call). Workspace-scoped: a ws-B list can only see ws-B's edges (§7).

use lb_assets::list_related;
use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::MembersError;

/// The `member` relation kind — the same string `assets::add_member` writes and
/// `visibility::may_read_doc` resolves. Kept here so this service names the same edge.
pub const MEMBER: &str = "member";

/// Return every `user:…` in `team` within workspace `ws` for `principal`. Order is unspecified
/// (the membership is a set). Empty if the team has no members in this workspace.
pub async fn list_members(
    store: &Store,
    principal: &Principal,
    ws: &str,
    team: &str,
) -> Result<Vec<String>, MembersError> {
    authorize_tool(principal, ws, "members.list").map_err(|_| MembersError::Denied)?;
    let members = list_related(store, ws, MEMBER, team).await?;
    Ok(members)
}
