//! The ingest service error. A `Denied` carries no detail (which gate failed, or whether the
//! series exists) — an un-granted producer leaks nothing about what series exist (ingest scope,
//! §3.5). Mirrors `AssetError`/`WorkflowError`.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IngestError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The input was not a valid `Sample[]` / arguments for the verb.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
