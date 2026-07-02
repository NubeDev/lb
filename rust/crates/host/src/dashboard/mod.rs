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
//!   - the MCP bridge ([`call_dashboard_tool`]) — the one MCP contract over all of the above.
//!   - the demo seed ([`seed_iot_demo`]) — real `Sample`s + tags via the real ingest path.

mod authorize;
mod bounds;
mod delete;
mod error;
mod genui;
mod get;
mod list;
mod model;
mod save;
mod seed;
mod share;
mod store;
mod tool;
mod visibility;

pub use bounds::{MAX_OVERRIDES, MAX_TRANSFORMS};
pub use delete::dashboard_delete;
pub use error::DashboardError;
pub use get::dashboard_get;
pub use list::dashboard_list;
pub use model::{Action, Cell, Dashboard, DashboardSummary, Source, Target, Variable, Visibility};
pub use save::dashboard_save;
pub use seed::{seed_iot_demo, SeedReport};
pub use share::dashboard_share;
pub use tool::call_dashboard_tool;
