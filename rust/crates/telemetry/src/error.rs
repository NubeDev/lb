//! The telemetry service error. A `Denied` carries no detail — an un-granted caller leaks nothing
//! about whether telemetry exists (telemetry-console scope, the operator-sink vs tenant-wall risk).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TelemetryError {
    /// Authorization failed (workspace isolation or the missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("bad input: {0}")]
    BadInput(String),
}
