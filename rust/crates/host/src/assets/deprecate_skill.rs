//! Deprecate a skill — CRUD's delete, done as a **soft delete** (core-skills scope, "User delete =
//! deprecate"). Versions are immutable and rollback-bearing, so a hard delete is wrong: it would
//! break rollback + audit and could yank a version a run is mid-way through. Instead the id is
//! hidden — it disappears from `list_skills`/latest resolution and the next run's catalog — while a
//! pinned `load_skill(id, version)` of an old version still resolves.
//!
//! Authorization: `store:skill/{id}:write` (the author-tier write cap — deprecating your own skill
//! is a write on it). Two gates before the flag write:
//!   - the `core.*` reservation (rejected regardless of caps — a core skill is never deprecatable);
//!   - the standard workspace + capability gate (`authorize_skill(..., Write)`).
//!
//! Re-publishing a new version un-hides (handled in `put_skill` — the flag is cleared on publish).

use lb_assets::{is_core, set_deprecated};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_skill;
use super::error::AssetError;

/// Deprecate skill `id` in workspace `ws` as `principal` (soft-hide from list/latest). Idempotent.
/// Rejects a `core.*` id (reserved) before the capability gate; then requires the skill write cap.
pub async fn deprecate_skill(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), AssetError> {
    if is_core(id) {
        return Err(AssetError::Reserved);
    }
    authorize_skill(principal, ws, id, Action::Write)?;
    set_deprecated(store, ws, id, true).await?;
    Ok(())
}
