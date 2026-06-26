//! Publish a skill version into the workspace namespace.
//!
//! A version is **immutable**: publishing `{id}@{version}` when that exact version already
//! exists is rejected (skills scope risk — a published version must not be silently
//! overwritten). A *new* version of the same `id` is a normal write (it lands at a distinct
//! `{id}@{version}` row). Namespace-scoped (README §7); raw verb.

use lb_store::{read, write, Store, StoreError};

use super::model::Skill;
use super::{skill_id, TABLE};

/// Publish `skill` (at `skill.id@skill.version`) into workspace `ws`. Errors if that exact
/// version already exists — versions are immutable. A different version of the same id is fine.
pub async fn put_skill(store: &Store, ws: &str, skill: &Skill) -> Result<(), StoreError> {
    let rid = skill_id(&skill.id, &skill.version);
    if read(store, ws, TABLE, &rid).await?.is_some() {
        return Err(StoreError::Decode(format!(
            "skill version already published, immutable: {rid}"
        )));
    }
    let value = serde_json::to_value(skill).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &rid, &value).await
}
