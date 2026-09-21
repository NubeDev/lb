//! `list_members` — every user in a team, for the members/teams UI.
//!
//! Gated by `mcp:members.list:call` (workspace-first). Reads the live `member` edges via
//! `team_member_edges`, so a revoked membership disappears immediately (the edges are re-resolved
//! every call). Workspace-scoped: a ws-B list can only see ws-B's edges (§7).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use crate::insight::team_member_edges;

use super::error::MembersError;

/// The `member` relation kind — the same string `assets::add_member` writes and
/// `visibility::may_read_doc` resolves. Kept here so this service names the same edge.
pub const MEMBER: &str = "member";

/// Return every `user:…` in `team` within workspace `ws` for `principal`. Order is unspecified
/// (the membership is a set). Empty if the team has no members in this workspace.
///
/// Resolves the team id under BOTH spellings via [`team_member_edges`] — see that function for why
/// `mechanical` and `team:mechanical` are both real ids. This verb shipped reading only the literal
/// string it was handed, so an MCP client that sent the prefixed form got `{"members":[]}` for a
/// team that plainly had members: no error, no denial, just a silent empty set that reads as "the
/// team is empty" rather than "you spelled the id the other way". `case.assignees` already resolved
/// both spellings, so the picker and this verb disagreed about the same team — which is exactly the
/// class of split the shared helper exists to prevent.
pub async fn list_members(
    store: &Store,
    principal: &Principal,
    ws: &str,
    team: &str,
) -> Result<Vec<String>, MembersError> {
    authorize_tool(principal, ws, "members.list").map_err(|_| MembersError::Denied)?;
    let members = team_member_edges(store, ws, team).await?;
    Ok(members)
}
