//! `template.get(id)` — read one durable scripted template (its code) in the workspace. Gated by
//! `mcp:template.get:call` (workspace-first). A template is workspace-shared (any member with the get
//! cap may read it, so a teammate can re-use a saved Plot/JSX snippet); the author-only check applies
//! to writes, not reads. Absent/tombstoned → `NotFound` (reachable only after gates 1+2 pass).

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_template;
use super::error::RenderTemplateError;
use super::model::RenderTemplate;
use super::store::read_template;

/// Read `render_template:{id}` in `ws` as `principal`. Returns the full record (incl. code).
pub async fn template_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<RenderTemplate, RenderTemplateError> {
    authorize_template(principal, ws, "template.get")?;
    match read_template(store, ws, id).await?.filter(|t| !t.deleted) {
        Some(t) => Ok(t),
        None => Err(RenderTemplateError::NotFound),
    }
}
