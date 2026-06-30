//! The identity service error — `Denied` is opaque (§3.5), like every service's denial. `Store` is
//! the durable-store failure. Identity verbs are admin-only and never carry secrets, so there is no
//! distinct `BadInput`/`Disabled` variant here.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
