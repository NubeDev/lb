//! `template.list()` — the workspace's durable scripted-template roster (summaries, no code bodies).
//! Gated by `mcp:template.list:call` (workspace-first). Workspace-scoped: a ws-A list sees only ws-A
//! templates (the hard wall); tombstones are dropped. The builder shows this as the "saved templates"
//! picker, then fetches a chosen template's code via `template.get`.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_template;
use super::error::RenderTemplateError;
use super::model::RenderTemplateSummary;
use super::store::scan_templates;

/// List the (non-tombstoned) templates in `ws` as summaries (id/title/engine/author/updated_ts).
pub async fn template_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<RenderTemplateSummary>, RenderTemplateError> {
    authorize_template(principal, ws, "template.list")?;
    let rows = scan_templates(store, ws).await?;
    Ok(rows
        .iter()
        .filter(|t| !t.deleted)
        .map(RenderTemplateSummary::from)
        .collect())
}
