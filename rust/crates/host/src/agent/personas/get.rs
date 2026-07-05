//! `agent.persona.get {id}` (member) — one persona. A `builtin.*` id resolves against the reserved
//! `_lb_personas` namespace (the same read-only union for every workspace); any other id resolves
//! against the caller's workspace namespace (the hard wall — a ws-B admin never sees a ws-A custom
//! persona). Gated by `mcp:agent.persona.get:call`. `NotFound` if the id is absent in its namespace.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};

use super::model::{is_builtin, Persona};
use super::store::{get_persona, PERSONA_NS};
use crate::boot::Node;

/// Read one persona by id. Built-ins come from the reserved namespace; custom from `ws`.
pub async fn agent_persona_get(
    node: &Node,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Persona, ToolError> {
    authorize_tool(principal, ws, "agent.persona.get").map_err(|_| ToolError::Denied)?;

    let (ns, builtin) = if is_builtin(id) {
        (PERSONA_NS, true)
    } else {
        (ws, false)
    };
    get_persona(&node.store, ns, id, builtin)
        .await
        .map_err(|_| ToolError::Denied)?
        .ok_or(ToolError::NotFound)
}
