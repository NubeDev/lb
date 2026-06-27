//! The DB-browser service error. A `Denied` carries no detail — an un-granted (or non-admin) caller
//! leaks nothing about what tables/records exist (data-console scope, the gate-3-relaxation risk).
//! Mirrors `IngestError`/`AssetError`.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbViewError {
    /// Authorization failed (workspace isolation or the missing admin capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The durable store rejected the read.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
