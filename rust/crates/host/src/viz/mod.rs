//! The viz panel-data resolver — the host's `viz.query` verb + its capability chokepoint (viz
//! transformations + datasource-binding scopes). `viz.query(panel) -> { frames, rows }` dispatches a
//! panel's targets under the caller's authority (composing each target tool's own cap + the workspace
//! wall by RE-ENTERING the generic MCP dispatcher — no render-path bypass), assembles canonical
//! frames, runs the `transformations[]` pipeline via the pure `lb-viz` lib, and returns the frames
//! (canonical) plus the primary frame flattened to the renderer's row shape.
//!
//! The files (one responsibility, FILE-LAYOUT):
//!   - `authorize.rs` ([`authorize_viz`]) — the `mcp:viz.query:call` verb gate.
//!   - `frame.rs` — a tool result `Value` → rows (mirrors the client `useSource.toRows`).
//!   - `query.rs` ([`viz_query`]) — the resolver: dispatch targets → assemble frames → run `lb-viz`.
//!   - `batch.rs` ([`viz_query_batch`]) — the `viz.query_batch` fan-in: resolve many panels in ONE call,
//!     concurrently, per-item partial failure (dashboard-query-acceleration scope, slice 3).
//!   - `time_override.rs` — the panel `timeFrom`/`timeShift` override applied to target args
//!     (grafana-parity-backend P1; Grafana's `applyPanelTimeOverrides` semantics, pinned there).
//!   - `tool.rs` ([`call_viz_tool`]) — the MCP bridge over `viz.*`.
//!   - `error.rs` ([`VizError`]) — opaque-deny error.

mod authorize;
mod batch;
mod error;
mod frame;
mod macros;
mod query;
mod resolution;
mod time_override;
mod tool;

pub use error::VizError;
pub use query::viz_query;
pub use tool::call_viz_tool;

/// The panel's dispatched target tools — reused by the gateway `subject_scoped` cache's capability
/// fingerprint so it folds EXACTLY the caps that gate this panel (dashboard-query-acceleration slice 2).
#[cfg(feature = "page-cache")]
pub(crate) use query::panel_target_tools;
