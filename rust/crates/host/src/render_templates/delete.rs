//! `template.delete(id)` — idempotent tombstone (author-only). Gated by `mcp:template.delete:call`.
//! Soft-delete (§6.8): the row stays with `deleted = true` so the upsert replays idempotently on the
//! sync path; `list`/`get` hide it. Deleting an absent/already-tombstoned id is a no-op success
//! (idempotent). A non-author with the delete cap cannot tombstone another author's template.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_template;
use super::error::RenderTemplateError;
use super::store::{read_template, write_template};

/// Tombstone `render_template:{id}` in `ws` as `principal`, at logical time `now`. Author-only;
/// idempotent (absent/already-deleted → `Ok`).
pub async fn template_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<(), RenderTemplateError> {
    authorize_template(principal, ws, "template.delete")?;
    match read_template(store, ws, id).await? {
        Some(existing) if !existing.deleted => {
            if existing.author != principal.sub() {
                return Err(RenderTemplateError::Denied);
            }
            let tombstone = super::model::RenderTemplate {
                deleted: true,
                updated_ts: now,
                ..existing
            };
            write_template(store, ws, &tombstone).await?;
            Ok(())
        }
        // Absent or already tombstoned — idempotent no-op (don't resurrect to write a tombstone).
        _ => Ok(()),
    }
}
