//! The agent service's error type. Mirrors the channel/asset error discipline: a denial is opaque
//! (no existence signal), store errors carry through, and a malformed invocation is `BadInput`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    /// A gate refused (the MCP invoke gate, or a tool/skill/doc the derived principal lacked).
    /// Opaque on purpose — the caller cannot tell "not allowed" from "absent".
    #[error("denied")]
    Denied,
    /// The session (job) was not found in this workspace — e.g. a resume of a missing/cross-ws job.
    #[error("session not found")]
    NotFound,
    /// The invocation arguments were malformed.
    #[error("bad input: {0}")]
    BadInput(String),
    /// A store operation failed underneath.
    #[error("store error: {0}")]
    Store(#[from] lb_store::StoreError),
}
