//! Load a skill — the grant-gated read verb (skills scope, README §6.12 "load only when
//! granted"). Three gates in order:
//!   1. workspace + 2. capability — `authorize_skill(..., Read)` (`store:skill/{id}:read`);
//!   3. grant — a live `grant:skill/{id}` relation for the workspace.
//!
//! A caller failing gate 1/2 gets `Denied` before any fetch. A caller passing 1+2 but with NO
//! grant for the skill also gets `Denied` — the mandatory deny (the S4 exit gate: "a skill loads
//! only when granted"). Version resolution: an explicit version, else the latest published.

use lb_assets::{get_skill, list_skills, related, Skill};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_skill;
use super::error::AssetError;
use super::grant_skill::{GRANT, GRANT_SCOPE};

/// Load skill `id` (at `version`, or latest if `None`) in workspace `ws` for `principal`, only
/// if the capability AND the workspace grant both hold.
pub async fn load_skill(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    version: Option<&str>,
) -> Result<Skill, AssetError> {
    // Gates 1 + 2: workspace isolation, then the skill read capability — before any fetch.
    authorize_skill(principal, ws, id, Action::Read)?;

    // Gate 3: the workspace must have granted this skill. No grant → denied (invisible).
    if !related(store, ws, GRANT, id, GRANT_SCOPE).await? {
        return Err(AssetError::Denied);
    }

    // Resolve the version: explicit, or the latest published (last by ts).
    match version {
        Some(v) => get_skill(store, ws, id, v)
            .await?
            .ok_or(AssetError::NotFound),
        None => list_skills(store, ws, id)
            .await?
            .pop()
            .ok_or(AssetError::NotFound),
    }
}
