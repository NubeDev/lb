//! The reserved **system scope** for core skills (core-skills scope, "Decided: a reserved system
//! scope in `lb-store` … skills-only, not a magic workspace id and not a second namespace"). Core
//! skills are ordinary `skill:{id}@{version}` records, but they live once in a node-level reserved
//! namespace instead of per-workspace — so `core.lb-cli` is seeded ONCE per node and resolves the
//! same for every workspace, after that workspace's grant check.
//!
//! This is the ONE skills-only resolver file the scope calls for. It holds:
//!   - the reserved namespace constant [`CORE_SKILLS_NS`] and the `core.` id prefix [`is_core`];
//!   - the seeder-only write [`seed_core_skill`] (idempotent — an existing immutable version is a
//!     no-op) — the boot seeder is the ONLY writer of this namespace;
//!   - the read verbs [`get_core_skill`] / [`list_core_skill_versions`] the host resolves `core.*`
//!     through, AFTER the workspace grant gate (the grant relation stays workspace-scoped).
//!
//! Mirrors the shipped reserved-namespace pattern (`_lb_identity` / `_lb_workspaces`): the leading
//! underscore marks it system-internal; an operator must never name a real workspace this.

use lb_store::{list as store_list, read, write, Store, StoreError};

use super::model::Skill;
use super::TABLE;

/// The reserved system namespace core skills live in. Not a real workspace — the leading underscore
/// marks it system-internal (the `_lb_identity` / `_lb_workspaces` convention). Only the boot seeder
/// writes here; workspace principals get read-only resolution of `core.*` through it.
pub const CORE_SKILLS_NS: &str = "_lb_skills";

/// The reserved id prefix that marks a skill as core (developer-authored, node-shipped, read-only to
/// users). `put_skill` / `deprecate_skill` reject any id with this prefix regardless of caps.
pub const CORE_PREFIX: &str = "core.";

/// True if `id` is in the reserved core namespace (`core.<name>`).
pub fn is_core(id: &str) -> bool {
    id.starts_with(CORE_PREFIX)
}

/// Seed one immutable core skill version into the reserved namespace. Idempotent: if
/// `core.<name>@<version>` already exists it is left untouched (immutable versions make a re-seed a
/// no-op — the boot seeder can run every boot). The ONLY writer of [`CORE_SKILLS_NS`].
pub async fn seed_core_skill(store: &Store, skill: &Skill) -> Result<(), StoreError> {
    let rid = super::skill_id(&skill.id, &skill.version);
    if read(store, CORE_SKILLS_NS, TABLE, &rid).await?.is_some() {
        return Ok(()); // Immutable version already seeded — no-op (idempotent re-seed).
    }
    let value = serde_json::to_value(skill).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, CORE_SKILLS_NS, TABLE, &rid, &value).await
}

/// Fetch a specific core skill version from the reserved namespace. `None` if that version was never
/// seeded (e.g. a rolled-back node build's version on a fresh store).
pub async fn get_core_skill(
    store: &Store,
    id: &str,
    version: &str,
) -> Result<Option<Skill>, StoreError> {
    match read(store, CORE_SKILLS_NS, TABLE, &super::skill_id(id, version)).await? {
        Some(value) => {
            let skill =
                serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(skill))
        }
        None => Ok(None),
    }
}

/// Every seeded version of core skill `id`, ordered by `ts` ascending (last = newest). Empty if the
/// id was never seeded. Mirrors [`list_skills`](super::list::list_skills) but against the reserved
/// namespace, so the host resolves a core skill's latest version the same way it resolves a user one.
pub async fn list_core_skill_versions(store: &Store, id: &str) -> Result<Vec<Skill>, StoreError> {
    let rows = store_list(store, CORE_SKILLS_NS, TABLE, "skill_key", id).await?;
    let mut skills: Vec<Skill> = rows
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect::<Result<_, _>>()?;
    skills.sort_by_key(|s| s.ts);
    Ok(skills)
}
