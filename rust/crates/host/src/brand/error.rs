//! The brand service error. A `Denied` carries no detail (which gate failed, or whether the brand
//! exists) — an un-granted caller leaks nothing about what brands exist (mirrors `PanelError`).
//! `NotFound` only ever reaches a caller who already passed gates 1+2.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BrandError {
    /// Authorization failed — workspace isolation or a missing capability. Opaque by design.
    #[error("denied")]
    Denied,
    /// The brand does not exist (or is tombstoned) — reachable only after gates 1+2 pass.
    #[error("not found")]
    NotFound,
    /// The input was not valid arguments for the verb.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
