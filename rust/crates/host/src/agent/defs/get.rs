//! `agent.def.get {id}` (member) — one catalog entry. A `builtin.*` id resolves against the reserved
//! `_lb_agents` namespace (the same read-only union for every workspace); any other id resolves
//! against the caller's workspace namespace (the hard wall — a ws-B admin never sees a ws-A custom
//! definition). Gated by `mcp:agent.def.get:call`. `NotFound` if the id is absent in its namespace.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};

use super::model::{is_builtin, AgentDefinition};
use super::store::{get_definition, AGENT_DEFS_NS};
use crate::boot::Node;

/// Read one definition by id. Built-ins come from the reserved namespace; custom from `ws`.
pub async fn agent_def_get(
    node: &Node,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<AgentDefinition, ToolError> {
    authorize_tool(principal, ws, "agent.def.get").map_err(|_| ToolError::Denied)?;

    let (ns, builtin) = if is_builtin(id) {
        (AGENT_DEFS_NS, true)
    } else {
        (ws, false)
    };
    get_definition(&node.store, ns, id, builtin)
        .await
        .map_err(|_| ToolError::Denied)?
        .ok_or(ToolError::NotFound)
}
