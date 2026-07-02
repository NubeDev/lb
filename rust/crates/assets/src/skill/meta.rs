//! Per-skill soft-delete metadata — the `skill_meta:{id}` record backing `deprecate_skill`
//! (core-skills scope, "User delete = deprecate"). Versions are immutable and rollback-bearing, so a
//! skill is never hard-deleted; instead a `deprecated` flag hides its id from `list`/`latest`
//! resolution while pinned loads of old versions still resolve (audit + rollback preserved).
//!
//! One flag record per skill id (NOT per version) in the workspace namespace — the id is what a user
//! deprecates. **Re-publishing a new version un-hides** (deprecate is a state, not a tombstone —
//! decided): `put_skill` clears the flag, so a fresh version brings the id back into the catalog.

use serde::{Deserialize, Serialize};

use lb_store::{read, write, Store, StoreError};

/// The store table the per-id soft-delete flags live in, within a workspace namespace.
pub(crate) const META_TABLE: &str = "skill_meta";

/// The soft-delete flag for one skill id. Absent record ⇒ not deprecated (the common case), so the
/// table only holds ids a user actually deprecated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillMeta {
    /// The skill id this flag is for (`acme-runbook`).
    pub id: String,
    /// Hidden from `list_skills`/latest resolution when true; a pinned `load_skill(id, version)`
    /// still resolves (rollback/audit).
    pub deprecated: bool,
}

/// Mark skill `id` deprecated (or un-deprecated) in workspace `ws`. Idempotent on the id.
pub async fn set_deprecated(
    store: &Store,
    ws: &str,
    id: &str,
    deprecated: bool,
) -> Result<(), StoreError> {
    let meta = SkillMeta {
        id: id.to_string(),
        deprecated,
    };
    let value = serde_json::to_value(&meta).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, META_TABLE, id, &value).await
}

/// True if skill `id` is currently deprecated in workspace `ws` (an absent flag record ⇒ false).
pub async fn is_deprecated(store: &Store, ws: &str, id: &str) -> Result<bool, StoreError> {
    match read(store, ws, META_TABLE, id).await? {
        Some(value) => {
            let meta: SkillMeta =
                serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(meta.deprecated)
        }
        None => Ok(false),
    }
}
