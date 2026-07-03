//! Shared write-time validation for the custom-definition verbs (agent-catalog scope). Two walls,
//! applied in this order on every create/update/delete:
//!   1. **Reserved tier (before the caps gate).** A `builtin.*` id is read-only to users regardless of
//!      caps — rejected `BadInput` (the core-skills `Reserved → BadInput` rule). Checked first so no
//!      broad grant can reach past it.
//!   2. **Runtime validation (on create/update).** A definition's `runtime` must be one the node's
//!      registry offers — an unrunnable id is `BadInput`, never a silent accept (the shipped
//!      `agent.config.set` rule). Keeps a custom definition in agreement with what the node can run.

use lb_mcp::ToolError;

use super::model::is_builtin;
use crate::boot::Node;

/// Reject a reserved `builtin.*` id on a write — read-only tier, checked BEFORE the caps gate. A
/// non-builtin id passes.
pub fn reject_reserved(id: &str) -> Result<(), ToolError> {
    if is_builtin(id) {
        return Err(ToolError::BadInput(format!(
            "{id:?} is a reserved built-in definition (read-only to users)"
        )));
    }
    Ok(())
}

/// Validate `runtime` against the node's registry — an id the node cannot run is a `BadInput`.
pub fn validate_runtime(node: &Node, runtime: &str) -> Result<(), ToolError> {
    let registry = node.runtimes();
    if !registry.ids().iter().any(|id| id == runtime) {
        return Err(ToolError::BadInput(format!(
            "unknown runtime {runtime:?} (node offers: {:?})",
            registry.ids()
        )));
    }
    Ok(())
}
