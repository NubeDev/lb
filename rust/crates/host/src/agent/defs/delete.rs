//! `agent.def.delete {id}` (ADMIN) — remove a **custom** workspace definition. Gated by
//! `mcp:agent.def.delete:call`.
//!
//! Ordered walls: (1) a `builtin.*` id is `BadInput` before the caps gate (read-only tier — a built-in
//! is never deletable); (2) caps gate; (3) the delete runs against the workspace namespace only, so a
//! ws-B admin can never erase a ws-A custom definition (the hard wall). A delete of an absent id is a
//! no-op success (idempotent).

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};

use super::store::delete_definition;
use super::validate::reject_reserved;
use crate::boot::Node;

/// Delete a custom definition `id` from `ws`.
pub async fn agent_def_delete(
    node: &Node,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), ToolError> {
    // (1) Reserved tier, before the caps gate — a built-in is read-only.
    reject_reserved(id)?;
    // (2) Caps gate.
    authorize_tool(principal, ws, "agent.def.delete").map_err(|_| ToolError::Denied)?;
    // (3) Namespace-scoped delete — the hard wall.
    delete_definition(&node.store, ws, id)
        .await
        .map_err(|_| ToolError::Denied)
}
