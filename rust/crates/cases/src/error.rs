//! The cases crate error — `Denied` is the host layer's concern (opaque, §3.5); the pure-verb layer
//! surfaces the store + validation outcomes the host maps to `ToolError`. Kept aligned with
//! `InsightsError`'s shape so the host wrapper is uniform.

use lb_store::StoreError;
use thiserror::Error;

/// A verb-layer error from `lb-cases`. The host service maps `Denied` (its own capability gate) and
/// any `BadInput` here into the opaque `ToolError` surface; everything else is a store or decode
/// failure.
#[derive(Debug, Error)]
pub enum CasesError {
    /// The caller-supplied input was malformed or violated an invariant — `resolved` without a
    /// resolution, a snooze with no reason, an insight already held by another OPEN case. Surfaced
    /// (not opaque) so the caller can correct the request and retry.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}

impl CasesError {
    /// Wrap a serde failure as a decode-flavoured store error — the shape every verb here uses
    /// when a record will not serialize.
    pub(crate) fn decode(e: impl std::fmt::Display) -> Self {
        CasesError::Store(StoreError::Decode(e.to_string()))
    }
}
