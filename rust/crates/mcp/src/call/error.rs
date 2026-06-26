//! The MCP tool-call error. `Denied` deliberately carries no detail about which gate failed
//! or whether the tool exists — an unauthorized caller learns nothing (mcp scope).

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolError {
    /// Authorization failed (workspace isolation or missing capability). No further detail by
    /// design — does not reveal tool existence.
    #[error("denied")]
    Denied,
    /// The qualified tool name is malformed or not hosted here. Only reachable by an
    /// already-authorized caller.
    #[error("no such tool")]
    NotFound,
    /// The extension ran but returned an error or trapped.
    #[error("extension error: {0}")]
    Extension(String),
    /// The input was not valid for the tool.
    #[error("bad input: {0}")]
    BadInput(String),
}
