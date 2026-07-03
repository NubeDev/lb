//! The panel service error. A `Denied` carries no detail (which gate failed, or whether the panel
//! exists) — an un-granted or non-member caller leaks nothing about what panels exist (library-panels
//! scope, mirrors `DashboardError`). `NotFound` only ever reaches a caller who already passed gates
//! 1+2 (so it is not an existence oracle to an outsider). `InUse` carries the referencing dashboards
//! so `panel.delete` can refuse a delete-in-use with the list (delete-safety).

use lb_store::StoreError;
use thiserror::Error;

use super::model::PanelUsageRow;

#[derive(Debug, Error)]
pub enum PanelError {
    /// Authorization failed — workspace isolation, a missing capability, or the gate-3 membership
    /// check (a non-member reading a team-shared panel). Opaque by design.
    #[error("denied")]
    Denied,
    /// The panel does not exist (or is tombstoned) — reachable only after gates 1+2 pass.
    #[error("not found")]
    NotFound,
    /// The input was not a valid panel / arguments for the verb (e.g. an over-cap spec).
    #[error("bad input: {0}")]
    BadInput(String),
    /// `panel.delete` refused because dashboards still reference the panel and `force` was not set —
    /// the referencing dashboards are returned so the caller can decide (library-panels delete-safety).
    #[error("panel in use by {} dashboard(s)", .0.len())]
    InUse(Vec<PanelUsageRow>),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
