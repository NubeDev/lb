//! `agent.persona.list` (member) — the persona catalog the picker renders: the reserved built-ins ∪
//! the workspace's custom personas, each tagged `builtin: true|false`. Gated by
//! `mcp:agent.persona.list:call` (workspace-first, opaque deny).
//!
//! Unlike the agent-definition catalog, there is **no node-runnable filter** — a persona is pure data
//! (a tool/skill allow-list), reachable on any node; whether a *listed tool* is reachable is decided
//! at run assembly by the wall, never at list time. Built-ins first (by id), then custom (by id).

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};

use super::model::Persona;
use super::store::{list_personas, PERSONA_NS};
use crate::boot::Node;

/// List the persona catalog for `ws` as `principal`. Built-ins (reserved ns) ∪ custom (ws ns).
pub async fn agent_persona_list(
    node: &Node,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Persona>, ToolError> {
    authorize_tool(principal, ws, "agent.persona.list").map_err(|_| ToolError::Denied)?;

    let mut out: Vec<Persona> = list_personas(&node.store, PERSONA_NS, true)
        .await
        .map_err(|_| ToolError::Denied)?;

    let custom = list_personas(&node.store, ws, false)
        .await
        .map_err(|_| ToolError::Denied)?;
    out.extend(custom);

    Ok(out)
}
