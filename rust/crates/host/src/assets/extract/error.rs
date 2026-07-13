//! The extraction service error (doc-extraction scope). Like the asset gate, a `Denied` is opaque
//! (no gate detail). This is the *job-level* error — a whole-request failure (missing cap, bad
//! input). Per-**item** failures are NOT errors: they are recorded outcomes on the item
//! (`unsupported`/`failed`/`denied`) while the job itself completes (scope: "per-item failure on
//! corrupt input while the job completes").

use lb_mcp::ToolError;
use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtractSvcError {
    /// The caller lacks `mcp:docs.extract:call` (workspace-first, then the cap). Opaque.
    #[error("denied")]
    Denied,
    /// The request itself was malformed (no media ids, a bad option value). Non-opaque — the
    /// caller is told plainly so they can fix the call.
    #[error("bad request: {0}")]
    BadInput(String),
    /// The durable store rejected a write (job record / ledger / doc / edge).
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}

impl From<ExtractSvcError> for ToolError {
    fn from(e: ExtractSvcError) -> Self {
        match e {
            ExtractSvcError::Denied => ToolError::Denied,
            ExtractSvcError::BadInput(s) => ToolError::BadInput(s),
            ExtractSvcError::Store(s) => ToolError::Extension(s.to_string()),
        }
    }
}
