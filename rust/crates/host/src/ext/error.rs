//! The ext lifecycle service error — `Denied` is opaque (§3.5).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// An uploaded artifact failed verification (tampered, unsigned, or a foreign publisher key).
    /// Distinct from `Denied`: the caller may be fully granted and still hand over bad bytes — the
    /// signature gate is independent of the capability gate. Nothing is stored on this error.
    #[error("artifact failed verification")]
    Unverified,
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    /// A native lifecycle step (stop/unload) failed during uninstall/disable.
    #[error("native lifecycle error: {0}")]
    Native(String),
}
