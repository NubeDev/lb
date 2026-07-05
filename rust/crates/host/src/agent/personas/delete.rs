//! `agent.persona.delete {id}` (ADMIN) — remove a **custom** workspace persona. Gated by
//! `mcp:agent.persona.delete:call`.
//!
//! Ordered walls: (1) a `builtin.*` id is `BadInput` before the caps gate (read-only tier — a built-in
//! is never deletable); (2) caps gate; (3) the delete runs against the workspace namespace only, so a
//! ws-B admin can never erase a ws-A custom persona (the hard wall). A delete of an absent id is a
//! no-op success (idempotent).

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};

use super::store::delete_persona;
use super::validate::reject_reserved;
use crate::boot::Node;

/// Delete a custom persona `id` from `ws`.
pub async fn agent_persona_delete(
    node: &Node,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), ToolError> {
    // (1) Reserved tier, before the caps gate — a built-in is read-only.
    reject_reserved(id)?;
    // (2) Caps gate.
    authorize_tool(principal, ws, "agent.persona.delete").map_err(|_| ToolError::Denied)?;
    // (3) Namespace-scoped delete — the hard wall.
    delete_persona(&node.store, ws, id)
        .await
        .map_err(|_| ToolError::Denied)
}
