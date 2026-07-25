//! `forms.save(id, title, def)` — one idempotent UPSERT for create+update (forms scope, "MCP
//! surface"; a fresh id creates, an existing id updates — not two verbs). Synchronous (one small
//! record; not a job). Gated by `mcp:forms.save:call`.
//!
//! **Ownership on update:** a save against an existing form is allowed only for its owner — a
//! non-owner with the save cap still cannot overwrite someone else's form (mirrors `dashboard.save`).
//! Create stamps `owner = principal`.

use lb_auth::Principal;
use lb_mcp::ToolDescriptor;
use lb_store::Store;
use serde_json::Value;

use super::authorize::authorize_form;
use super::error::FormError;
use super::model::Form;
use super::store::{read_form, write_form};

/// The `forms.save` descriptor — a real arg schema so a model advertised the verb can FORM the call
/// (the same reason `dashboard.save` carries one). `def` is typed `object` loudly; its inner shape is
/// described, not enumerated — the definition is opaque to the host beyond serde.
pub fn save_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "forms.save".to_string(),
        title: "Create or update a form (idempotent upsert)".to_string(),
        group: "forms".to_string(),
        input_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "x-lb": { "label": "Form id", "description": "Fresh id creates; existing id updates (owner-only)" } },
                "title": { "type": "string", "x-lb": { "label": "Title" } },
                "def": { "type": "object", "x-lb": { "label": "Definition", "description": "The form definition (the options.form shape): schema, ui, submit, mode, recordSource, optionsSources, success. Pass a JSON OBJECT, never a JSON-encoded string." } },
                "now": { "type": "integer", "x-lb": { "label": "Timestamp", "description": "Logical time of the save — unix epoch seconds" } }
            },
            "required": ["id", "title", "def", "now"]
        })),
        result: None,
    }
}

/// Upsert form `id` in `ws` with `title` + `def`, as `principal`, at logical time `now`. Creates on a
/// fresh id (owner = the principal's `owner_sub` — the human behind a derived agent actor, so an
/// agent-built form belongs to whoever asked); updates an existing one (owner-only). Returns the
/// persisted record.
pub async fn forms_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    def: Value,
    now: u64,
) -> Result<Form, FormError> {
    authorize_form(principal, ws, "forms.save")?;
    if id.is_empty() {
        return Err(FormError::BadInput("empty form id".into()));
    }

    // Preserve owner across an update; only the owner may update. A tombstoned record is treated as
    // absent — a save with that id resurrects it under the new owner (create).
    let owner = match read_form(store, ws, id).await?.filter(|f| !f.deleted) {
        Some(existing) => {
            if existing.owner != principal.owner_sub() {
                return Err(FormError::Denied);
            }
            existing.owner
        }
        None => principal.owner_sub().to_string(),
    };

    let form = Form {
        id: id.to_string(),
        title: title.to_string(),
        def,
        owner,
        schema_version: super::model::SCHEMA_VERSION,
        updated_ts: now,
        deleted: false,
    };
    write_form(store, ws, &form).await?;
    Ok(form)
}
