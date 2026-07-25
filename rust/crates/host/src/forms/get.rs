//! `forms.get(id)` — the read verb (forms scope, "MCP surface"). Gates 1+2 (`authorize_form`) run
//! before any fetch (no existence signal to an outsider), then fetch. There is NO gate-3 visibility
//! check — a form is a simple owner/workspace asset, so a workspace member holding the cap may read it
//! (unlike a dashboard, which carries a per-record visibility tier). A tombstoned form reads as
//! `NotFound`.

use lb_auth::Principal;
use lb_mcp::ToolDescriptor;
use lb_store::Store;

use super::authorize::authorize_form;
use super::error::FormError;
use super::model::Form;
use super::store::read_form;

/// The `forms.get` descriptor — a real arg schema so a model can form the call.
pub fn get_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "forms.get".to_string(),
        title: "Read one form by id".to_string(),
        group: "forms".to_string(),
        input_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "x-lb": { "label": "Form id" } }
            },
            "required": ["id"]
        })),
        result: None,
    }
}

/// Read form `id` in `ws` for `principal`, if gates 1+2 pass. A tombstoned/absent form is `NotFound`.
pub async fn forms_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Form, FormError> {
    // Gates 1 + 2: workspace isolation, then the read capability — before any fetch.
    authorize_form(principal, ws, "forms.get")?;

    read_form(store, ws, id)
        .await?
        .filter(|f| !f.deleted)
        .ok_or(FormError::NotFound)
}
