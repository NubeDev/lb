//! `members.remove` — the missing destructive member verb (admin-crud scope). Removes the
//! `team -[member]-> user` edge, gated by `mcp:teams.manage:call` (the admin cap, not the S4
//! doc-write stopgap), workspace-first. Idempotent: removing an absent membership is a success.
//!
//! The freshness asymmetry bites here and the UI must surface it: dropping the edge revokes the
//! user's access to resources shared to that team **live** (Gate 3 re-resolves the edge every read),
//! but caps the user **inherited** via the team stay in their current token until **re-mint**
//! (Gate 2). For a true immediate lockout, pair with `membership.remove` (which marks the live token)
//! + a short TTL — the legacy `user.disable` flag no longer exists (pre-production legacy sweep).

use lb_assets::unrelate;
use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::MembersError;
use super::list::MEMBER;

/// Remove `user` from `team` in workspace `ws` as `principal`. Idempotent; workspace-first.
///
/// Drops the edge under BOTH spellings of the team id. A team record stores whatever id it was
/// created with verbatim, so `mechanical` and `team:mechanical` are both real and the product
/// writes both (see `insight::team_member_edges`). Unrelating only the literal string handed in
/// makes a remove that targets the other spelling a silent success: the verb returns `Ok`, the UI
/// reports the member gone, and the edge is still there — the user keeps team-granted read access
/// with nothing to show why. Idempotency makes that indistinguishable from a genuine no-op, which
/// is what makes it worth removing both rather than probing first.
pub async fn remove_member(
    store: &Store,
    principal: &Principal,
    ws: &str,
    team: &str,
    user: &str,
) -> Result<(), MembersError> {
    authorize_tool(principal, ws, "teams.manage").map_err(|_| MembersError::Denied)?;
    unrelate(store, ws, MEMBER, team, user).await?;
    // The other spelling. Unconditional: a membership can exist under both ids at once (the admin
    // console writes one, the pickers the other), and leaving either behind is the bug above.
    let other = match team.strip_prefix("team:") {
        Some(bare) => bare.to_string(),
        None => format!("team:{team}"),
    };
    unrelate(store, ws, MEMBER, &other, user).await?;
    crate::authz::invalidate_entity_scope(ws);
    Ok(())
}
