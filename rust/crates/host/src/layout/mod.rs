//! The ui-layout service (data-studio scope v2, "Layout persistence") — a tiny MEMBER-OWNED record
//! holding a dockable surface's layout JSON: `ui_layout:[ws, user, surface]`. The nav-pref pattern
//! generalized: the record is always keyed by the authenticated principal's `sub` (a caller can never
//! read or write another user's layout), the workspace is the hard wall (rule 6), and the `surface`
//! key is **opaque data** (rule 10 — any dockable surface, core or extension-hosted, persists here
//! without the host knowing which). The layout is a LENS on the client's window arrangement — it
//! grants nothing and is never interpreted server-side (bounded opaque JSON, like a dashboard's
//! `options`).
//!
//! Deliberately NOT an `lb-prefs` axis (that axis set is closed to formatting) and NOT an
//! `assets.put_doc` doc (doc ids are workspace-global — two members would clobber one layout).
//!
//! The verbs (one per file, FILE-LAYOUT):
//!   - `layout.get` ([`layout_get`]) — read the caller's OWN layout for a surface.
//!   - `layout.set` ([`layout_set`]) — upsert the caller's OWN layout for a surface (LWW).
//!   - the MCP bridge ([`call_layout_tool`]) — the one MCP contract over both.

mod error;
mod get;
mod model;
mod set;
mod store;
mod tool;

pub use error::LayoutError;
pub use get::layout_get;
pub use model::{UiLayout, MAX_LAYOUT_BYTES};
pub use set::layout_set;
pub use tool::call_layout_tool;
