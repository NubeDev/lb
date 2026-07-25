//! `forms.list()` — the roster verb (forms scope, "Get / list"). Returns every form in the workspace
//! (a form is a simple owner/workspace asset — no gate-3 visibility filter), as cheap summaries
//! (id/title/updated_ts, **no definition body**). Gates 1+2 first, then scan, drop tombstones, map.

use lb_auth::Principal;
use lb_mcp::ToolDescriptor;
use lb_store::Store;

use super::authorize::authorize_form;
use super::error::FormError;
use super::model::FormSummary;
use super::store::scan_forms;

/// The `forms.list` descriptor — a real (empty-arg) schema so a model can form the call.
pub fn list_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "forms.list".to_string(),
        title: "List the forms in the workspace (summaries)".to_string(),
        group: "forms".to_string(),
        input_schema: Some(serde_json::json!({ "type": "object", "properties": {} })),
        result: None,
    }
}

/// List the forms in `ws`. Tombstoned forms are excluded.
pub async fn forms_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<FormSummary>, FormError> {
    authorize_form(principal, ws, "forms.list")?;

    let all = scan_forms(store, ws).await?;
    let out = all
        .iter()
        .filter(|f| !f.deleted)
        .map(FormSummary::from)
        .collect();
    Ok(out)
}
