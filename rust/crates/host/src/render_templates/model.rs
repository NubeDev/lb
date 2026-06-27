//! The `render_template` record — a durable, workspace-scoped, author-owned scripted-view snippet
//! (widget-builder scope, "Data (SurrealDB)"). A scripted view (`plot`/`d3`/`template`) whose code is
//! larger than the inline `cell.options` cap lives here as a `render_template:{id}` row, not in
//! `localStorage` (code is state → SurrealDB, rule 2/4). Small snippets stay inline in the cell; the
//! threshold is [`INLINE_MAX_BYTES`] (widget-builder open Q3, lean: a few KB).
//!
//! The cell references the durable template by id; the row holds the actual code. Author-owned: only
//! the author may update/delete their template (mirrors the dashboard owner check).

use serde::{Deserialize, Serialize};

/// The table durable scripted templates live in. Record id is `render_template:{id}` (a stable slug,
/// unique per workspace).
pub const TABLE: &str = "render_template";

/// The inline-vs-row threshold (widget-builder open Q3). A scripted snippet at or below this size may
/// live inline in `cell.options.code`; larger code MUST become a `render_template:{id}` row. The
/// builder enforces this; the host bounds the row body too (a few KB keeps the dashboard record lean).
pub const INLINE_MAX_BYTES: usize = 4 * 1024;

/// The hard ceiling on a single template body (defense in depth — a runaway snippet can't bloat the
/// store unboundedly). Generous over [`INLINE_MAX_BYTES`] since a durable template is *meant* to be
/// the larger case, but still bounded.
pub const TEMPLATE_MAX_BYTES: usize = 64 * 1024;

/// The render engine a template's `code` targets — the scripted-view trust tier renders all of these
/// in a sandboxed iframe (widget-builder scope, "Scripted views").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    /// A JSX render template over the source rows.
    Template,
    /// An Observable Plot snippet over the source rows.
    Plot,
    /// A D3 / Observable snippet over the source rows.
    D3,
}

/// A durable scripted-view template. The code is author-owned and workspace-scoped; a cell references
/// it by `id`. The host never executes it — it is rendered in the sandboxed iframe tier on the client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderTemplate {
    /// Stable slug, unique per workspace (the record id `render_template:{id}`).
    pub id: String,
    /// A human label (the builder's saved-template roster shows this).
    pub title: String,
    /// The render engine the `code` targets.
    pub engine: Engine,
    /// The scripted-view source code (JSX / Plot / D3). Bounded to [`TEMPLATE_MAX_BYTES`].
    pub code: String,
    /// The principal who authored it (only the author may update/delete — the owner check).
    pub author: String,
    pub updated_ts: u64,
    /// Tombstone (soft-delete, §6.8 idempotent). A deleted template is hidden from `list`/`get`.
    #[serde(default)]
    pub deleted: bool,
}

/// The cheap roster row `list` returns — id/title/engine/author/updated_ts, **no code body** (the
/// roster stays light; the builder fetches a template's code only when it's opened).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderTemplateSummary {
    pub id: String,
    pub title: String,
    pub engine: Engine,
    pub author: String,
    pub updated_ts: u64,
}

impl From<&RenderTemplate> for RenderTemplateSummary {
    fn from(t: &RenderTemplate) -> Self {
        Self {
            id: t.id.clone(),
            title: t.title.clone(),
            engine: t.engine,
            author: t.author.clone(),
            updated_ts: t.updated_ts,
        }
    }
}
