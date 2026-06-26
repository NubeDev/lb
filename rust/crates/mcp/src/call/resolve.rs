//! The resolve phase — map a qualified tool name `<ext>.<tool>` to its dispatch target (local
//! instance or remote node). Only reached after `authorize` passed, so a `NotFound` here is
//! never seen by an unauthorized caller (mcp scope).

use crate::registry::{Registry, Target};

use super::error::ToolError;

/// Split `<ext>.<tool>` and find the target hosting `<tool>` (local or remote). Returns an owned
/// [`Target`] (a cheap clone sharing the instance `Arc`) so it outlives the registry read lock.
pub fn resolve(registry: &Registry, qualified_tool: &str) -> Result<Target, ToolError> {
    let (ext_id, tool) = qualified_tool.split_once('.').ok_or(ToolError::NotFound)?;
    let target = registry.get(ext_id).ok_or(ToolError::NotFound)?;
    if !target.tools().iter().any(|t| t == tool) {
        return Err(ToolError::NotFound);
    }
    Ok(target)
}
