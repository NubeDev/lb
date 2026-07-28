//! The versions service error. `Denied` is opaque (§3.5) like every other service — a caller
//! refused `versions.get` learns nothing about whether the version exists.

use lb_mcp::ToolError;
use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VersionsError {
    /// Authorization failed — the verb's own cap, the kind's save cap (the no-escalation check on
    /// restore), or the admin gate on `versions.config.set`. Opaque by design.
    #[error("denied")]
    Denied,
    /// An unknown kind, a malformed argument, or a cap outside the node's clamp range. A typed
    /// error, never a silent store miss (the scope's catalog requirement).
    #[error("{0}")]
    BadInput(String),
    /// No such version for this entity in this workspace — including a version that belongs to a
    /// different entity, or that the ring has since evicted.
    #[error("no such version")]
    NotFound,
    /// The restore's re-dispatched save refused. Passed through VERBATIM (it is the save verb's own
    /// validator talking — "a snapshot made invalid by a since-tightened check is refused, not
    /// written"), so the caller sees why rather than a generic restore failure.
    #[error("restore refused by {tool}: {message}")]
    RestoreRefused { tool: String, message: String },
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}

impl From<VersionsError> for ToolError {
    fn from(e: VersionsError) -> Self {
        match e {
            VersionsError::Denied => ToolError::Denied,
            VersionsError::BadInput(m) => ToolError::BadInput(m),
            VersionsError::NotFound => ToolError::NotFound,
            // A refused restore is a BAD REQUEST about the snapshot the caller chose, not a server
            // fault: the stored version no longer satisfies a validator, and the caller's fix is to
            // pick a different version. `400` says that; `500` would read as "the node is broken".
            VersionsError::RestoreRefused { tool, message } => {
                ToolError::BadInput(format!("restore refused by {tool}: {message}"))
            }
            VersionsError::Store(s) => ToolError::Extension(s.to_string()),
        }
    }
}
