//! The membership service error — `Denied` is opaque (§3.5). `Store` is the durable-store failure.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MembershipError {
    /// Authorization failed (workspace isolation, missing capability, OR — for the login seam — the
    /// subject is not a member of a workspace that already has members). Opaque by design.
    #[error("denied")]
    Denied,
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
