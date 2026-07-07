//! The dashboard service — the host's capability chokepoint for the dashboard surface (dashboard
//! scope; README §6.6, the S4 asset model). A dashboard is an **asset**: a workspace-namespaced
//! `dashboard:{id}` record holding a grid layout (`cells[]`), wrapped with the three-gate read check
//! (workspace → capability → membership/visibility), reusing the shipped S4 `share`/`member` edges
//! rather than a new ACL.
//!
//! The verbs (one per file, FILE-LAYOUT):
//!   - `dashboard.get` ([`dashboard_get`]) — three-gate read of one dashboard.
//!   - `dashboard.list` ([`dashboard_list`]) — the membership-filtered roster (summaries, no cells).
//!   - `dashboard.save` ([`dashboard_save`]) — idempotent UPSERT for create+update (owner-only update).
//!   - `dashboard.delete` ([`dashboard_delete`]) — idempotent tombstone (owner-only).
//!   - `dashboard.share` ([`dashboard_share`]) — set visibility / write the S4 `share` edge.
//!   - `dashboard.pin` ([`dashboard_pin`]) — mint a persisted cell from an `x-lb-render` envelope
//!     (widget-platform scope, Slice B). Generic over the tool id; reuses the Slice A validation chain.
//!   - the MCP bridge ([`call_dashboard_tool`]) — the one MCP contract over all of the above.
//!   - the demo seed ([`seed_iot_demo`]) — real `Sample`s + tags via the real ingest path.

mod authorize;
mod bounds;
mod catalog;
mod delete;
mod error;
pub(crate) mod genui;
mod get;
mod list;
mod model;
mod pin;
mod save;
mod seed;
mod share;
mod store;
mod tool;
mod views;
mod visibility;

pub use bounds::{check_spec_bounds, MAX_OVERRIDES, MAX_TRANSFORMS};
pub use catalog::{catalog_descriptor, dashboard_catalog, ExtWidget, WidgetCatalog};
pub use delete::dashboard_delete;
pub use error::DashboardError;
pub use get::dashboard_get;
pub use list::dashboard_list;
pub use model::{Action, Cell, Dashboard, DashboardSummary, Source, Target, Variable, Visibility};
pub use pin::{dashboard_pin, mint_cell_from_envelope, pin_descriptor};
pub use save::{dashboard_save, save_descriptor};
pub use seed::{seed_iot_demo, SeedReport};
pub use share::{dashboard_share, share_descriptor};
pub use store::scan_dashboards;
pub use tool::call_dashboard_tool;
pub use views::{builtin_view_ids, check_view_cells};
pub use visibility::may_read_dashboard;
