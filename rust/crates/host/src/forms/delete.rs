//! `forms.delete(id)` — tombstone-upsert (forms scope, "MCP surface"; idempotent). A re-delete is a
//! no-op, and a delete of an absent form is a no-op (not an error) — the idempotency the sync path
//! relies on. Gated by `mcp:forms.delete:call`. Owner-only UNLESS the caller also holds
//! `mcp:forms.delete_any:call` (an admin-granted cap, checked second so a non-admin never pays its
//! cost) — an admin needs to clean up forms other members own. Mirrors `dashboard.delete` exactly.

use lb_auth::Principal;
use lb_mcp::ToolDescriptor;
use lb_store::Store;

use super::authorize::authorize_form;
use super::error::FormError;
use super::store::{read_form, write_form};

/// The `forms.delete` descriptor — a real arg schema so a model can form the call.
pub fn delete_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "forms.delete".to_string(),
        title: "Delete a form (idempotent tombstone)".to_string(),
        group: "forms".to_string(),
        input_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "x-lb": { "label": "Form id" } },
                "now": { "type": "integer", "x-lb": { "label": "Timestamp", "description": "Logical time of the delete — unix epoch seconds" } }
            },
            "required": ["id", "now"]
        })),
        result: None,
    }
}

/// Soft-delete form `id` in `ws` as `principal`, at logical time `now`. Idempotent: an absent or
/// already-tombstoned form is a no-op. The owner may always delete; a non-owner may delete only if
/// also granted `mcp:forms.delete_any:call` (admin override).
pub async fn forms_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<(), FormError> {
    authorize_form(principal, ws, "forms.delete")?;

    match read_form(store, ws, id).await? {
        // Already gone (absent or tombstoned) — idempotent no-op.
        None => Ok(()),
        Some(f) if f.deleted => Ok(()),
        Some(mut f) => {
            if f.owner != principal.owner_sub()
                && authorize_form(principal, ws, "forms.delete_any").is_err()
            {
                return Err(FormError::Denied);
            }
            f.deleted = true;
            f.updated_ts = now;
            write_form(store, ws, &f).await?;
            Ok(())
        }
    }
}
