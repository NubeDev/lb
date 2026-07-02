//! Publish a skill version into the workspace namespace.
//!
//! A version is **immutable**: publishing `{id}@{version}` when that exact version already
//! exists is rejected (skills scope risk — a published version must not be silently
//! overwritten). A *new* version of the same `id` is a normal write (it lands at a distinct
//! `{id}@{version}` row). Namespace-scoped (README §7); raw verb.

use lb_store::{read, write, Store, StoreError};

use super::meta::set_deprecated;
use super::model::Skill;
use super::{skill_id, TABLE};

/// Publish `skill` (at `skill.id@skill.version`) into workspace `ws`. Errors if that exact
/// version already exists — versions are immutable. A different version of the same id is fine.
///
/// Publishing a new version also **un-hides** a previously-deprecated id (core-skills scope
/// decision: "a new version un-hides it — deprecate is a state, not a tombstone"). This clears the
/// `skill_meta` flag so the freshly-published version reappears in `list_skills`/latest resolution.
pub async fn put_skill(store: &Store, ws: &str, skill: &Skill) -> Result<(), StoreError> {
    let rid = skill_id(&skill.id, &skill.version);
    if read(store, ws, TABLE, &rid).await?.is_some() {
        return Err(StoreError::Decode(format!(
            "skill version already published, immutable: {rid}"
        )));
    }
    let value = serde_json::to_value(skill).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &rid, &value).await?;
    // Un-hide: a re-publish of a deprecated id brings it back (clear the soft-delete flag).
    set_deprecated(store, ws, &skill.id, false).await
}
