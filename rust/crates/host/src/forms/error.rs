//! The form service error. A `Denied` carries no detail (which gate failed, or whether the form
//! exists) — an un-granted caller leaks nothing about what forms exist (mirrors `DashboardError`).
//! `NotFound` only ever reaches a caller who already passed gates 1+2 (so it is not an existence
//! oracle to an outsider).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FormError {
    /// Authorization failed — workspace isolation or a missing capability. Opaque by design.
    #[error("denied")]
    Denied,
    /// The form does not exist (or is tombstoned) — reachable only after gates 1+2 pass.
    #[error("not found")]
    NotFound,
    /// The input was not a valid form / arguments for the verb.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
