//! Add a user to a team — write a `member` relation edge. The membership the doc-sharing gate
//! resolves (`visibility::may_read_doc`). At S4 this is gated by `store:doc/*:write` (a
//! workspace-admin-ish act); a dedicated `teams.add_member` capability/role is the follow-up
//! (files scope open question). It exists now so the share→read flow (and its tests) have a
//! way to populate teams.

use lb_assets::relate;
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_doc;
use super::error::AssetError;
use super::visibility::MEMBER;

/// Add `user` to `team` in workspace `ws`. Idempotent. Gated by the doc-surface write cap
/// (`*` resource) as an admin act at S4.
pub async fn add_member(
    store: &Store,
    principal: &Principal,
    ws: &str,
    team: &str,
    user: &str,
) -> Result<(), AssetError> {
    authorize_doc(principal, ws, "*", Action::Write)?;
    relate(store, ws, MEMBER, team, user).await?;
    Ok(())
}
