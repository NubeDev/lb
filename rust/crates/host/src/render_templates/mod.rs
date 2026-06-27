//! The render-template service — the host's capability chokepoint for durable scripted-view templates
//! (widget-builder scope, "Data (SurrealDB)"). A scripted view (`plot`/`d3`/`template`) whose code is
//! larger than the inline `cell.options` cap persists as a workspace-scoped, author-owned
//! `render_template:{id}` record — code is state, so SurrealDB, never `localStorage` (rule 2/4). A cell
//! references the template by id; the host never executes it (it renders in the client's sandboxed
//! iframe tier).
//!
//! The verbs (one per file, FILE-LAYOUT) — the full CRUD the scope named:
//!   - `template.save`   ([`template_save`])   — idempotent UPSERT for create+update (author-only update).
//!   - `template.get`    ([`template_get`])    — read one template (its code), workspace-shared.
//!   - `template.list`   ([`template_list`])   — the workspace roster (summaries, no code bodies).
//!   - `template.delete` ([`template_delete`]) — idempotent tombstone (author-only).
//!   - the MCP bridge ([`call_template_tool`]) — the one MCP contract over all of the above.

mod authorize;
mod delete;
mod error;
mod get;
mod list;
mod model;
mod save;
mod store;
mod tool;

pub use delete::template_delete;
pub use error::RenderTemplateError;
pub use get::template_get;
pub use list::template_list;
pub use model::{
    Engine, RenderTemplate, RenderTemplateSummary, INLINE_MAX_BYTES, TEMPLATE_MAX_BYTES,
};
pub use save::template_save;
pub use tool::call_template_tool;
