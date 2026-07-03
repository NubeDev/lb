//! The layout service error. `Denied` is opaque (which gate failed leaks nothing); there is no
//! `NotFound` — an absent layout is a `Default` payload, not an error (the client falls back to its
//! built-in layout, mirroring `nav.pref`'s absent-pick behavior).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LayoutError {
    /// Authorization failed — workspace isolation or a missing capability. Opaque by design.
    #[error("denied")]
    Denied,
    /// The input was not valid (an empty surface key, an over-cap model).
    #[error("bad input: {0}")]
    BadInput(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
