//! The render-template service error. A `Denied` carries no detail (which gate failed, or whether a
//! template exists) — an un-granted caller leaks nothing (widget-builder scope; mirrors
//! `DashboardError`). `NotFound` only reaches a caller who already passed gates 1+2.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderTemplateError {
    /// Authorization failed — workspace isolation, a missing capability, or the author-ownership
    /// check on update/delete. Opaque by design.
    #[error("denied")]
    Denied,
    /// The template does not exist (or is tombstoned) — reachable only after gates 1+2 pass.
    #[error("not found")]
    NotFound,
    /// The input was not a valid template / arguments for the verb (e.g. code over the size cap).
    #[error("bad input: {0}")]
    BadInput(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
