//! `template.save(id, title, engine, code)` — one idempotent UPSERT for create+update (widget-builder
//! scope). A fresh id creates (author = principal); an existing id updates (author-only). Gated by
//! `mcp:template.save:call`. Bounded: `code` over [`TEMPLATE_MAX_BYTES`] is rejected (a runaway
//! snippet can't bloat the store). Author-owned: a non-author with the save cap still cannot overwrite
//! another author's template (mirrors `dashboard_save`'s owner check).

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_template;
use super::error::RenderTemplateError;
use super::model::{Engine, RenderTemplate, TEMPLATE_MAX_BYTES};
use super::store::{read_template, write_template};

/// Upsert `render_template:{id}` in `ws` with `title`/`engine`/`code`, as `principal`, at logical time
/// `now`. Creates on a fresh id (author = principal); updates an existing one (author-only). Returns
/// the persisted record.
#[allow(clippy::too_many_arguments)]
pub async fn template_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    engine: Engine,
    code: &str,
    now: u64,
) -> Result<RenderTemplate, RenderTemplateError> {
    authorize_template(principal, ws, "template.save")?;
    if id.is_empty() {
        return Err(RenderTemplateError::BadInput("empty template id".into()));
    }
    if code.len() > TEMPLATE_MAX_BYTES {
        return Err(RenderTemplateError::BadInput(format!(
            "template code over the {TEMPLATE_MAX_BYTES}-byte cap"
        )));
    }

    // Preserve author across an update; only the author may update. A tombstoned record is treated as
    // absent — a save with that id resurrects it under the new author (create).
    let author = match read_template(store, ws, id).await?.filter(|t| !t.deleted) {
        Some(existing) => {
            if existing.author != principal.sub() {
                return Err(RenderTemplateError::Denied);
            }
            existing.author
        }
        None => principal.sub().to_string(),
    };

    let template = RenderTemplate {
        id: id.to_string(),
        title: title.to_string(),
        engine,
        code: code.to_string(),
        author,
        updated_ts: now,
        deleted: false,
    };
    write_template(store, ws, &template).await?;
    Ok(template)
}
