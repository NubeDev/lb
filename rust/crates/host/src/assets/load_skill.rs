//! Load a skill — the grant-gated read verb (skills scope, README §6.12 "load only when
//! granted"). Three gates in order:
//!   1. workspace + 2. capability — `authorize_skill(..., Read)` (`store:skill/{id}:read`);
//!   3. grant — a live `grant:skill/{id}` relation for the workspace.
//!
//! A caller failing gate 1/2 gets `Denied` before any fetch. A caller passing 1+2 but with NO
//! grant for the skill also gets `Denied` — the mandatory deny (the S4 exit gate: "a skill loads
//! only when granted"). Version resolution: an explicit version, else the latest published.
//!
//! **Two tiers, one gate (core-skills scope).** A `core.*` id resolves against the reserved system
//! namespace (`list_core_skill_versions`/`get_core_skill`), a user id against the workspace
//! namespace — but the grant gate (3) is IDENTICAL and workspace-scoped for both. So a core skill
//! present on every node still loads only when the workspace granted it (no core bypass — the
//! headline deny). A deprecated user id is hidden from LATEST resolution but a pinned version still
//! loads (rollback/audit); core ids are never deprecatable.

use lb_assets::{
    get_core_skill, get_skill, is_core, is_deprecated, list_core_skill_versions, list_skills,
    related, Skill,
};
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

    // Gate 3: the workspace must have granted this skill. No grant → denied (invisible). Identical
    // for a core and a user skill — the grant relation is workspace-scoped for BOTH (no core bypass).
    if !related(store, ws, GRANT, id, GRANT_SCOPE).await? {
        return Err(AssetError::Denied);
    }

    // Tier split is on WHERE the record lives, not on the gate: core ids resolve against the reserved
    // system namespace, user ids against the workspace namespace.
    if is_core(id) {
        return match version {
            Some(v) => get_core_skill(store, id, v)
                .await?
                .ok_or(AssetError::NotFound),
            None => list_core_skill_versions(store, id)
                .await?
                .pop()
                .ok_or(AssetError::NotFound),
        };
    }

    // User tier. A pinned version always resolves (rollback/audit — even for a deprecated id); LATEST
    // resolution honors the soft-delete flag (a deprecated id has no visible latest).
    match version {
        Some(v) => get_skill(store, ws, id, v)
            .await?
            .ok_or(AssetError::NotFound),
        None => {
            if is_deprecated(store, ws, id).await? {
                return Err(AssetError::NotFound);
            }
            list_skills(store, ws, id)
                .await?
                .pop()
                .ok_or(AssetError::NotFound)
        }
    }
}
