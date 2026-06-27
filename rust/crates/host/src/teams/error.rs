//! The teams service error — `Denied` is opaque (§3.5).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TeamsError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
