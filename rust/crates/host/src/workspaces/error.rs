//! The workspaces service error — `Denied` is opaque (§3.5).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspacesError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    /// The workspace id/name failed validation (provision scope).
    #[error("invalid: {0}")]
    Invalid(String),
    /// The workspace was hard-deleted; a tombstone is never resurrected (admin-crud scope).
    #[error("workspace was purged")]
    Purged,
    /// A provision write failed at `stage` (`plan` | `bootstrap` | `directory`). The bootstrap batch
    /// is atomic and the directory row is written last, so the workspace is absent from the
    /// directory — nothing partially-provisioned is listable (workspace-provision scope).
    #[error("provision failed at {stage}: {source}")]
    ProvisionFailed {
        stage: &'static str,
        #[source]
        source: StoreError,
    },
}
