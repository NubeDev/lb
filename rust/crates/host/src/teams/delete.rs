//! `teams.delete` — delete a team and cascade its membership + grants (admin-crud scope). Gated by
//! `mcp:teams.manage:call`, workspace-first. The resolved open question: **cascade-remove** the
//! member edges + revoke the team's grants in one logical op (the UI shows the member count before
//! confirm), rather than blocking on a non-empty team.
//!
//! Three effects, idempotent and workspace-first:
//!   1. drop every `member` edge `team -[member]-> user` (Gate-3 access via the team drops **live**);
//!   2. revoke the team's grants via the slice-1 authz seam (`revoke_subject` — team-inherited caps
//!      drop on members' **re-mint**, the freshness asymmetry the UI surfaces);
//!   3. tombstone the team record so it leaves the listing (no row-delete; sync-idempotent).
//!
//! Deleting an absent/empty team is a success; never crosses the workspace wall.

use lb_assets::{list_related, unrelate};
use lb_auth::Principal;
use lb_authz::{revoke_subject, team_delete, Subject, MEMBER};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::TeamsError;

/// Delete `team` in workspace `ws` as `principal`, cascading edges + grants. Returns the count of
/// members removed (for the UI consequence note). Idempotent.
pub async fn teams_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    team: &str,
) -> Result<usize, TeamsError> {
    authorize_tool(principal, ws, "teams.manage").map_err(|_| TeamsError::Denied)?;

    // 1. Cascade-remove the membership edges (live Gate-3 drop).
    let members = list_related(store, ws, MEMBER, team).await?;
    for user in &members {
        unrelate(store, ws, MEMBER, team, user).await?;
    }

    // 2. Revoke the team's own grants (slice-1 seam) — inherited caps drop on members' re-mint.
    revoke_subject(store, ws, &Subject::Team(team.to_string())).await?;

    // 3. Tombstone the team record so it leaves the listing.
    team_delete(store, ws, team).await?;

    Ok(members.len())
}
