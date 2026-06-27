//! The authz service error — `Denied` is opaque (no detail about which gate failed, §3.5), like
//! every other service's denial. `Widen` guards the no-widening rule (a definer can only bundle
//! caps they themselves hold) — it is surfaced as a bad-input, not a leak.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthzError {
    /// Authorization failed (workspace isolation or missing admin capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// A `roles.define` tried to bundle a cap the definer does not hold (the no-widening rule).
    #[error("cannot grant a capability you do not hold: {0}")]
    Widen(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
