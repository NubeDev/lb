//! Errors for the `chains.*` host service.

use lb_mcp::ToolError;
use lb_rules::workflow::DagError;

#[derive(thiserror::Error, Debug)]
pub enum ChainsError {
    #[error("denied")]
    Denied,
    #[error("not found")]
    NotFound,
    #[error("invalid DAG: {0}")]
    Dag(#[from] DagError),
    #[error("bad input: {0}")]
    BadInput(String),
    #[error("{0}")]
    Internal(String),
}

impl From<ChainsError> for ToolError {
    fn from(e: ChainsError) -> Self {
        match e {
            ChainsError::Denied => ToolError::Denied,
            ChainsError::NotFound => ToolError::NotFound,
            // A bad DAG is author feedback (a 400-equivalent), not an auth signal.
            ChainsError::Dag(d) => ToolError::BadInput(d.to_string()),
            ChainsError::BadInput(m) => ToolError::BadInput(m),
            ChainsError::Internal(m) => ToolError::Extension(m),
        }
    }
}
