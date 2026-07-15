//! The store-admin surface error. `Denied` is opaque (which gate failed — workspace,
//! capability — leaks nothing; mirrors `StoreQueryError`).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreAdminError {
    /// Authorization failed — workspace isolation or a missing `store:status:read` /
    /// `store:compact:run`. Opaque by design.
    #[error("denied")]
    Denied,
    /// The durable store rejected the operation (a runtime fault, a timeout).
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
