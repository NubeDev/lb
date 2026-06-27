//! The tags service error. A `Denied` carries no detail (which gate failed, or whether a tag/entity
//! exists) — `tags.find` without the grant cannot even enumerate tag existence (tags scope, §3.5).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TagsError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The per-workspace tag-node cap was exceeded (a new distinct tag node was refused).
    #[error("tag-node cap exceeded (cap {0})")]
    CapExceeded(usize),
    /// The input was not valid for the verb.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
