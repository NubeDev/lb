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
    /// The payload exceeded the v1 inline-asset size bound (document-store scope risk). Never
    /// raised silently — a clear, honest rejection so a caller knows streaming/buckets is the
    /// path for larger blobs.
    #[error("asset too large")]
    TooLarge,
    /// A write targeted the reserved `core.*` skill namespace (core-skills scope). Unlike `Denied`
    /// this is NOT opaque — the `core.` namespace is a public, deliberate reservation, so the caller
    /// is told plainly that core skills change only by shipping a new node build (not a caps signal).
    #[error("reserved namespace: core skills are read-only to users")]
    Reserved,
}
