//! The prefs host-service error. A capability denial is **opaque** ([`PrefsSvcError::Denied`]) — it
//! leaks nothing about whether the record exists. Pure-layer failures (unknown unit, cross-dimension,
//! bad instant) surface as `BadInput` so a caller can fix its request; a store failure is `Store`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrefsSvcError {
    /// Authorization failed (workspace wall or capability) — opaque, no existence signal.
    #[error("denied")]
    Denied,
    /// A pure-layer (conversion/format/parse) failure the caller can correct.
    #[error("bad input: {0}")]
    BadInput(String),
    /// An underlying store failure.
    #[error("store: {0}")]
    Store(#[from] lb_store::StoreError),
}
