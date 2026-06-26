//! List every published version of a skill `id`, newest version last (by `ts`).
//!
//! Backed by the denormalized `skill_key` filter, so it is one namespace-scoped `store::list`
//! (README §7). The host uses this to resolve "latest granted version" (skills scope) — it
//! picks from this list; the store does not order, so the `ts` sort is here.

use lb_store::{list as store_list, Store, StoreError};

use super::model::Skill;
use super::TABLE;

/// Return every version of skill `id` in workspace `ws`, ordered by `ts` ascending (so the
/// last element is the newest published version). Empty if the id has no versions here.
pub async fn list_skills(store: &Store, ws: &str, id: &str) -> Result<Vec<Skill>, StoreError> {
    let rows = store_list(store, ws, TABLE, "skill_key", id).await?;
    let mut skills: Vec<Skill> = rows
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect::<Result<_, _>>()?;
    skills.sort_by_key(|s| s.ts);
    Ok(skills)
}
