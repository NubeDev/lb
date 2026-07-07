//! The panel service — the host's capability chokepoint for the **library-panels** surface
//! (library-panels scope; README §6.5, the S4 asset model). A panel is an **asset** — the `dashboard`
//! pattern cloned one level down: a workspace-namespaced `panel:{id}` record holding the **non-layout
//! half of a v3 `Cell`** (the reusable spec), wrapped with the three-gate read check (workspace →
//! capability → membership/visibility), reusing the shipped S4 `share`/`member` edges rather than a
//! new ACL.
//!
//! **A panel is a LENS over data access, never a grant.** Sharing a panel shares its *definition*; the
//! `sources[]` it reads still re-check under the **viewer's** caps per call (the "sharing never widens
//! data access" headline). Its record read passes the three gates; its data reads pass `viz.query`'s
//! per-target leash unchanged.
//!
//! The verbs (one per file, FILE-LAYOUT):
//!   - `panel.get` ([`panel_get`]) — three-gate read of one panel (full spec).
//!   - `panel.list` ([`panel_list`]) — the membership-filtered roster (summaries, no spec).
//!   - `panel.save` ([`panel_save`]) — idempotent UPSERT for create+update (owner-only update; bounded).
//!   - `panel.delete` ([`panel_delete`]) — idempotent tombstone (owner-only; delete-safety on refs).
//!   - `panel.share` ([`panel_share`]) — set visibility / write the S4 `share` edge.
//!   - `panel.usage` ([`panel_usage`]) — which dashboards reference a panel (delete-safety + banner).
//!   - the MCP bridge ([`call_panel_tool`]) — the one MCP contract over all of the above.
//!
//! Two seams the *dashboard* verbs call (the ref lifecycle, host-side per the scope Decision):
//!   - [`hydrate_cells`] — `dashboard.get` expands each ref cell's `panel_ref` → resolved v3 cell.
//!   - [`validate_and_strip_refs`] — `dashboard.save` validates refs resolve + strips the echoed spec.

mod authorize;
mod bounds;
mod delete;
mod error;
mod get;
mod hydrate;
mod list;
mod model;
mod save;
mod share;
mod store;
mod tool;
mod usage;
mod validate;
mod visibility;

pub use delete::panel_delete;
pub use error::PanelError;
pub use get::panel_get;
pub use hydrate::hydrate_cells;
pub use list::panel_list;
pub use model::{Panel, PanelSpec, PanelSummary, PanelUsageRow, Visibility, SCHEMA_VERSION};
pub use save::panel_save;
pub use share::panel_share;
pub use store::{read_panel, write_panel};
pub use tool::call_panel_tool;
pub use usage::panel_usage;
pub use validate::validate_and_strip_refs;
// Crate-internal: the spec-bounds check `panel.save` runs — `dashboard.pin` reuses it on its
// panel-write so a pinned envelope is bounded by the same per-record limit as a hand-authored one.
pub(crate) use bounds::check_spec_bounds;
