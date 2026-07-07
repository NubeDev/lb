//! `agent.persona.list` (member) — the persona catalog the picker renders: the reserved built-ins ∪
//! the workspace's custom personas, each tagged `builtin: true|false` and `enabled: true|false`
//! (computed against the `agent.config.enabled_personas` roster — persona-session #5, open-question 1:
//! the picker and the dock's context matcher need ONE fetch, no client-side roster join; the raw
//! roster still rides `agent.config.get` for the Settings editor). Gated by
//! `mcp:agent.persona.list:call` (workspace-first, opaque deny).
//!
//! Unlike the agent-definition catalog, there is **no node-runnable filter** — a persona is pure data
//! (a tool/skill allow-list), reachable on any node; whether a *listed tool* is reachable is decided
//! at run assembly by the wall, never at list time. Built-ins first (by id), then custom (by id) —
//! the deterministic order the dock's multi-match suggestion leans on (first enabled match wins).

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde::{Deserialize, Serialize};

use super::model::Persona;
use super::resolve::is_enabled;
use super::store::{list_personas, PERSONA_NS};
use crate::agent::get_agent_config;
use crate::boot::Node;

/// One catalog row: the whole persona record plus its roster state. `enabled` is advertisement-layer
/// curation (a disabled persona is hidden from pickers + context matching, and an explicit invoke of
/// it fails with a named error) — the capability wall beneath is unchanged either way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonaListItem {
    #[serde(flatten)]
    pub persona: Persona,
    pub enabled: bool,
}

impl std::ops::Deref for PersonaListItem {
    type Target = Persona;
    fn deref(&self) -> &Persona {
        &self.persona
    }
}

/// List the persona catalog for `ws` as `principal`. Built-ins (reserved ns) ∪ custom (ws ns), each
/// flagged against the workspace roster (`None`/empty roster = all enabled).
pub async fn agent_persona_list(
    node: &Node,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<PersonaListItem>, ToolError> {
    authorize_tool(principal, ws, "agent.persona.list").map_err(|_| ToolError::Denied)?;

    let mut out: Vec<Persona> = list_personas(&node.store, PERSONA_NS, true)
        .await
        .map_err(|_| ToolError::Denied)?;

    let custom = list_personas(&node.store, ws, false)
        .await
        .map_err(|_| ToolError::Denied)?;
    out.extend(custom);

    let cfg = get_agent_config(&node.store, ws)
        .await
        .map_err(|_| ToolError::Denied)?;
    let roster = cfg.as_ref().and_then(|c| c.enabled_personas.as_ref());

    Ok(out
        .into_iter()
        .map(|persona| {
            let enabled = is_enabled(roster, &persona.id);
            PersonaListItem { persona, enabled }
        })
        .collect())
}
