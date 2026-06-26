//! The members service error — `Denied` is opaque (no detail about which gate failed, §3.5), like
//! every other service's denial.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MembersError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
