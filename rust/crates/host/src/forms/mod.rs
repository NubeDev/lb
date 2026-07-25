//! The forms service — the host's capability chokepoint for the form surface (forms scope, the asset
//! model). A form is an **asset**: a workspace-namespaced `form:{id}` record holding a typed
//! definition (`def` — the `options.form` shape). It mirrors the [`dashboard`](crate::dashboard)
//! family EXACTLY, simplified: a form is a simple owner/workspace asset, so it needs no
//! bounds/genui/views/share/visibility/catalog/pin — the definition is opaque to the host beyond
//! serde, so the verbs just authorize, then persist/read.
//!
//! The verbs (one per file, FILE-LAYOUT):
//!   - `forms.get` ([`forms_get`]) — read one form (gates 1+2; no gate-3, a form has no visibility tier).
//!   - `forms.list` ([`forms_list`]) — the workspace roster (summaries, no definition body).
//!   - `forms.save` ([`forms_save`]) — idempotent UPSERT for create+update (owner-only update).
//!   - `forms.delete` ([`forms_delete`]) — idempotent tombstone (owner-only, admin override).
//!   - the MCP bridge ([`call_forms_tool`]) — the one MCP contract over all of the above.

mod authorize;
mod delete;
mod error;
mod get;
mod list;
mod model;
mod save;
mod store;
mod tool;

pub use delete::{delete_descriptor, forms_delete};
pub use error::FormError;
pub use get::{forms_get, get_descriptor};
pub use list::{forms_list, list_descriptor};
pub use model::{Form, FormSummary};
pub use save::{forms_save, save_descriptor};
pub use tool::call_forms_tool;
