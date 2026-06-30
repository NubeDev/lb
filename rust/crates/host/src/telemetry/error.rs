//! The telemetry service error. A `Denied` carries no detail — an un-granted caller learns nothing
//! about whether telemetry exists (telemetry-console scope, the operator-sink vs tenant-wall risk).
//! Mirrors `DbViewError`/`QueryError`.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TelemetrySvcError {
    /// Authorization failed (workspace isolation or the missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("bad input: {0}")]
    BadInput(String),
}
