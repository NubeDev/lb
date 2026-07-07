//! The insights crate error — `Denied` is the host layer's concern (opaque, §3.5); the pure-verb
//! layer surfaces the store + validation outcomes the host maps to `ToolError`. Kept aligned with
//! `lb_inbox`/`lb_outbox`'s error shapes so the host wrapper is uniform.

use lb_store::StoreError;
use thiserror::Error;

/// A verb-layer error from `lb-insights`. The host service maps `Denied` (its own capability
/// gate) and any `BadInput` here into the opaque `ToolError` surface; everything else is a store
/// or decode failure.
#[derive(Debug, Error)]
pub enum InsightsError {
    /// The caller-supplied input was malformed or violated a hard limit (an oversize
    /// occurrence `data`, a too-large subscription filter, an out-of-bounds policy override).
    /// Surfaced (not opaque) so the producer can slim its payload and retry.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
