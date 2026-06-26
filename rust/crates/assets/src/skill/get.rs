//! Load one skill version by `(id, version)` from the workspace namespace.
//!
//! The namespace is selected from `ws`, so a load for workspace A returns `None` for a skill
//! in workspace B (README §7). Raw verb: the host checks the workspace **grant** for the skill
//! *before* calling this — an ungranted skill never reaches here (skills scope).

use lb_store::{read, Store, StoreError};

use super::model::Skill;
use super::{skill_id, TABLE};

/// Fetch `{id}@{version}` from workspace `ws`. `None` if that version is absent here.
pub async fn get_skill(
    store: &Store,
    ws: &str,
    id: &str,
    version: &str,
) -> Result<Option<Skill>, StoreError> {
    match read(store, ws, TABLE, &skill_id(id, version)).await? {
        Some(value) => {
            let skill =
                serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(skill))
        }
        None => Ok(None),
    }
}
