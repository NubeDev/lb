//! Publish a skill version — the write verb. Requires `store:skill/{id}:write`, workspace-first.
//! Versions are immutable (the store verb rejects re-publishing an existing `{id}@{version}` —
//! skills scope), so this is publish-new, not overwrite.

use lb_assets::{put_skill as store_put_skill, Skill};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_skill;
use super::error::AssetError;

/// Publish skill `id`@`version` in workspace `ws` as `principal`. Errors if that exact version
/// already exists (immutable).
#[allow(clippy::too_many_arguments)]
pub async fn put_skill(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    version: &str,
    description: &str,
    body: &str,
    ts: u64,
) -> Result<Skill, AssetError> {
    authorize_skill(principal, ws, id, Action::Write)?;
    let skill = Skill::new(id, version, principal.sub(), description, body, ts);
    store_put_skill(store, ws, &skill).await?;
    Ok(skill)
}
