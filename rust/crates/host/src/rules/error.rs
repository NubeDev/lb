//! Errors for the `rules.*` host service, mapped onto `ToolError` at the MCP bridge.

use lb_mcp::ToolError;

#[derive(thiserror::Error, Debug)]
pub enum RulesError {
    #[error("denied")]
    Denied,
    #[error("not found")]
    NotFound,
    /// A user-script fault (author feedback — surfaced, not opaque).
    #[error("{0}")]
    Eval(String),
    #[error("bad input: {0}")]
    BadInput(String),
    #[error("{0}")]
    Internal(String),
}

impl From<RulesError> for ToolError {
    fn from(e: RulesError) -> Self {
        match e {
            RulesError::Denied => ToolError::Denied,
            RulesError::NotFound => ToolError::NotFound,
            // A script eval fault is author feedback for the Playground, not an auth signal.
            RulesError::Eval(m) => ToolError::BadInput(m),
            RulesError::BadInput(m) => ToolError::BadInput(m),
            RulesError::Internal(m) => ToolError::Extension(m),
        }
    }
}
