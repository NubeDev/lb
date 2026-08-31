//! The report service — the host's capability chokepoint for the **report builder** surface (reports
//! scope). A report is an **asset** — the `dashboard`/`panel` pattern: a workspace-namespaced
//! `report:{id}` record holding an ordered `blocks[]` notebook (markdown / image / panel), an owner,
//! the S4 visibility tier, a `brand_id`, and an opaque report-level `toolbar`. Wrapped with the
//! three-gate read check (workspace → capability → membership/visibility), reusing the shipped S4
//! `share`/`member` edges.
//!
//! Panel blocks reuse the **shipped** ref lifecycle verbatim (reports scope Decision): `report.get`
//! hydrates each panel block's cell through `hydrate_cells`; `report.save` validates + strips refs
//! through `validate_and_strip_refs`. No new hydration code.
//!
//! The verbs (one per file, FILE-LAYOUT):
//!   - `report.get` ([`report_get`]) — three-gate read, panel blocks hydrated.
//!   - `report.list` ([`report_list`]) — the membership-filtered roster (summaries).
//!   - `report.save` ([`report_save`]) — idempotent UPSERT (owner-only update; block-capped; refs
//!     validated).
//!   - `report.delete` ([`report_delete`]) — idempotent tombstone (owner-only; plain soft-delete).
//!   - `report.share` ([`report_share`]) — set visibility / write the S4 `share` edge.
//!   - `report.export` ([`report_export`]) — assemble a report-kind DASHBOARD's cell grid + brand +
//!     client snapshots → branded PDF bytes. Gated on its own `report.export` cap.
//!   - `report.export` on the JSON bridge ([`report_export_media`]) — the same composition with
//!     **media ids on both ends**, for callers that cannot set an `Authorization` header (a
//!     module-federated extension UI). Adds no authorization: it calls [`report_export`].
//!   - the MCP bridge ([`call_report_tool`]) — get/list/save/delete/share **and** export.

mod authorize;
pub mod compose;
mod delete;
mod error;
mod export;
mod export_media;
mod get;
mod list;
mod model;
pub mod options;
mod profile;
mod rendered;
mod save;
mod share;
mod store;
mod tool;
mod visibility;

pub use delete::report_delete;
pub use error::ReportError;
pub use export::report_export;
pub use export_media::{report_export_media, REPORT_ORIGIN};
pub use get::report_get;
pub use list::report_list;
pub use model::{Block, Report, ReportSummary, Visibility as ReportVisibility, MAX_BLOCKS};
pub use options::ExportOptions;
pub use profile::ExportProfile;
pub use rendered::RenderedPanel;
pub use save::report_save;
pub use share::report_share;
pub use tool::call_report_tool;
