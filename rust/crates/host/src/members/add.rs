//! `add_member` — add a user to a team through a real, capability-gated verb (collaboration scope,
//! slice 3).
//!
//! Gated by `mcp:members.add:call` (workspace-first) — distinct from the S4 `assets::add_member`,
//! which gated this on the doc-write cap as a stopgap (files scope open question: "a dedicated
//! teams.add_member capability is the follow-up"). This is that dedicated capability. Idempotent on
//! `(team, user)` — re-adding upserts the same `member` edge. Workspace-scoped (§7): the edge lands
//! in `ws`'s namespace, so it can never cross the wall.

use lb_assets::relate;
use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::MembersError;
use super::list::MEMBER;

/// Add `user` to `team` in workspace `ws` as `principal`. Idempotent.
pub async fn add_member(
    store: &Store,
    principal: &Principal,
    ws: &str,
    team: &str,
    user: &str,
) -> Result<(), MembersError> {
    authorize_tool(principal, ws, "members.add").map_err(|_| MembersError::Denied)?;
    relate(store, ws, MEMBER, team, user).await?;
    Ok(())
}
