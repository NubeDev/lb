//! The resolve phase — map a qualified tool name `<ext>.<tool>` to the hosting extension.
//! Only reached after `authorize` passed, so a `NotFound` here is never seen by an
//! unauthorized caller (mcp scope).

use crate::registry::{Hosted, Registry};

use super::error::ToolError;

/// Split `<ext>.<tool>` and find the hosting extension that declares `<tool>`.
pub fn resolve<'r>(registry: &'r Registry, qualified_tool: &str) -> Result<&'r Hosted, ToolError> {
    let (ext_id, tool) = qualified_tool.split_once('.').ok_or(ToolError::NotFound)?;
    let hosted = registry.get(ext_id).ok_or(ToolError::NotFound)?;
    if !hosted.tools.iter().any(|t| t == tool) {
        return Err(ToolError::NotFound);
    }
    Ok(hosted)
}
