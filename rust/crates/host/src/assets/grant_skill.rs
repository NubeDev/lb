//! Grant / revoke a skill for the workspace — write/delete the `grant:skill/{id}` relation
//! (skills scope). This is the workspace saying "our agents may load this skill." It is
//! deliberately a relation, not a capability minted into a token: the workspace grants/revokes
//! by writing/deleting one record, and the next `load_skill` reflects it immediately.
//!
//! Authorization: `store:skill/{id}:write` — granting is a privileged (admin-ish) act gated by
//! the write capability on the skill surface. (A finer-grained `grant` capability/role is a
//! skills-scope open question for S5.)

use lb_assets::{relate, unrelate};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_skill;
use super::error::AssetError;

/// The relation kind + fixed `b` for a workspace skill grant. `a` is the skill id; `b` is the
/// constant `"ws"` (the grant is workspace-wide at S4 — team-scoped grants are a follow-up).
pub(crate) const GRANT: &str = "grant";
pub(crate) const GRANT_SCOPE: &str = "ws";

/// Grant skill `id` to the workspace `ws` (its agents may now load it). Idempotent.
pub async fn grant_skill(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), AssetError> {
    authorize_skill(principal, ws, id, Action::Write)?;
    relate(store, ws, GRANT, id, GRANT_SCOPE).await?;
    Ok(())
}

/// Revoke skill `id`'s grant in workspace `ws` (agents can no longer load it). Idempotent.
pub async fn revoke_skill(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), AssetError> {
    authorize_skill(principal, ws, id, Action::Write)?;
    unrelate(store, ws, GRANT, id, GRANT_SCOPE).await?;
    Ok(())
}
