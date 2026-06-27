//! The dashboard service error. A `Denied` carries no detail (which gate failed, or whether the
//! dashboard exists) — an un-granted or non-member caller leaks nothing about what dashboards exist
//! (dashboard scope, §3.5; mirrors `AssetError`/`IngestError`). `NotFound` only ever reaches a caller
//! who already passed gates 1+2 (so it is not an existence oracle to an outsider).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DashboardError {
    /// Authorization failed — workspace isolation, a missing capability, or the gate-3 membership
    /// check (a non-member reading a team-shared dashboard). Opaque by design.
    #[error("denied")]
    Denied,
    /// The dashboard does not exist (or is tombstoned) — reachable only after gates 1+2 pass.
    #[error("not found")]
    NotFound,
    /// The input was not a valid dashboard / arguments for the verb.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
