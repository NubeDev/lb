//! `members.remove` — the missing destructive member verb (admin-crud scope). Removes the
//! `team -[member]-> user` edge, gated by `mcp:teams.manage:call` (the admin cap, not the S4
//! doc-write stopgap), workspace-first. Idempotent: removing an absent membership is a success.
//!
//! The freshness asymmetry bites here and the UI must surface it: dropping the edge revokes the
//! user's access to resources shared to that team **live** (Gate 3 re-resolves the edge every read),
//! but caps the user **inherited** via the team stay in their current token until **re-mint**
//! (Gate 2). For a true immediate lockout, pair with `user.disable` + short TTL.

use lb_assets::unrelate;
use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::MembersError;
use super::list::MEMBER;

/// Remove `user` from `team` in workspace `ws` as `principal`. Idempotent; workspace-first.
pub async fn remove_member(
    store: &Store,
    principal: &Principal,
    ws: &str,
    team: &str,
    user: &str,
) -> Result<(), MembersError> {
    authorize_tool(principal, ws, "teams.manage").map_err(|_| MembersError::Denied)?;
    unrelate(store, ws, MEMBER, team, user).await?;
    Ok(())
}
