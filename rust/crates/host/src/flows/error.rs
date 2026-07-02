//! The `flows.*` service error type. A deny is opaque (`Denied`), so it is indistinguishable from a
//! missing flow at the MCP surface.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FlowsError {
    #[error("denied")]
    Denied,
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    BadInput(String),
    #[error("{0}")]
    Internal(String),
    /// A resume whose next-frontier nodes no longer match the pinned graph's type + ports
    /// (Decision 1). Surfaced read-side by `flows.runs.get`; the run fails cleanly, never silently
    /// mis-executes.
    #[error("resume point drift: {0}")]
    ResumePointDrift(String),
}

impl FlowsError {
    pub fn to_tool(self) -> lb_mcp::ToolError {
        match self {
            FlowsError::Denied | FlowsError::NotFound => lb_mcp::ToolError::Denied,
            FlowsError::BadInput(m) => lb_mcp::ToolError::BadInput(m),
            other => lb_mcp::ToolError::Extension(other.to_string()),
        }
    }
}
