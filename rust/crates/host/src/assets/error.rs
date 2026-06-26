//! The asset service error. Like the channel and MCP `Denied`, a `Denied` here carries no
//! detail about *which* gate failed (workspace, capability, or membership/grant) — a caller
//! without access learns nothing about the asset, not even that it exists (§3.5).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssetError {
    /// Authorization failed at some gate (workspace isolation, missing capability, or not a
    /// member / not granted). Opaque by design — no existence signal.
    #[error("denied")]
    Denied,
    /// The requested asset does not exist (only returned to a caller who PASSED the gates, so
    /// it leaks nothing — a denied caller gets `Denied`, never `NotFound`).
    #[error("not found")]
    NotFound,
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
