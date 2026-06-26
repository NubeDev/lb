//! The host `native` service error domain (native-tier scope). Spans the service — the gate, the
//! supervisor, the records, the manifest. The MCP bridge maps it onto `ToolError` at the edge.

use lb_store::StoreError;
use lb_supervisor::SupervisorError;
use thiserror::Error;

use crate::load::LoadError;

#[derive(Debug, Error)]
pub enum NativeServiceError {
    /// The MCP/workspace gate refused the caller. Opaque — no signal about whether the sidecar
    /// exists (same discipline as `ToolError::Denied`).
    #[error("denied")]
    Denied,
    /// The manifest is not a valid native manifest (not `tier="native"`, or missing `[native]`).
    #[error("not a native extension: {0}")]
    NotNative(String),
    /// No sidecar is running for this `(ws, ext_id)` — a lifecycle/call verb was asked of one that
    /// is not started here. Distinct from `Denied` (the caller may be fully granted).
    #[error("sidecar not running")]
    NotRunning,
    /// The supervisor failed (spawn, transport, restart budget). Surfaced so the caller sees a
    /// process fault distinctly from an authz failure.
    #[error("supervisor: {0}")]
    Supervisor(#[from] SupervisorError),
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("load error: {0}")]
    Load(#[from] LoadError),
}
