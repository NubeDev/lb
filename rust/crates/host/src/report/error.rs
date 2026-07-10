//! The report service error (mirrors `PanelError`). `Denied` carries no detail (which gate failed,
//! or whether the report exists). `NotFound` only ever reaches a caller who already passed gates
//! 1+2. `Render` wraps an export-assembly / Typst-compile failure.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReportError {
    /// Authorization failed — workspace isolation, a missing capability, or the gate-3 membership
    /// check (a non-member reading a team-shared report). Opaque by design.
    #[error("denied")]
    Denied,
    /// The report does not exist (or is tombstoned) — reachable only after gates 1+2 pass.
    #[error("not found")]
    NotFound,
    /// The input was not valid (e.g. over the block cap, or a dangling `panel_ref`).
    #[error("bad input: {0}")]
    BadInput(String),
    /// PDF export failed — assembly or the Typst compile (`lb_render::RenderError`).
    #[error("render error: {0}")]
    Render(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
