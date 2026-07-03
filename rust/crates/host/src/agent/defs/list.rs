//! `agent.def.list` (member) — the catalog the UI renders: the node-runnable built-ins ∪ the
//! workspace's custom definitions, each tagged `builtin: true|false`. Gated by
//! `mcp:agent.def.list:call` (workspace-first, opaque deny).
//!
//! **Node-runnable filter (symmetric, no `if cloud`).** A built-in whose `runtime` the node's registry
//! does not offer is omitted — a node without the `external-agent` feature seeds the
//! `open-interpreter.*` entries but never lists them, because `open-interpreter-default` is not in the
//! registry. Custom entries are shown even if their runtime drifted out of the registry (the UI flags
//! them, mirroring the Agent tab's stored-but-unavailable note), so an admin can see + fix them.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};

use super::model::AgentDefinition;
use super::store::{list_definitions, AGENT_DEFS_NS};
use crate::boot::Node;

/// List the catalog for `ws` as `principal`. Built-ins are filtered to node-runnable runtimes; custom
/// entries are all returned. Sorted: built-ins first (by id), then custom (by id).
pub async fn agent_def_list(
    node: &Node,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<AgentDefinition>, ToolError> {
    authorize_tool(principal, ws, "agent.def.list").map_err(|_| ToolError::Denied)?;

    let registry = node.runtimes();
    let runnable = |runtime: &str| registry.ids().iter().any(|id| id == runtime);

    let mut out: Vec<AgentDefinition> = list_definitions(&node.store, AGENT_DEFS_NS, true)
        .await
        .map_err(|_| ToolError::Denied)?
        .into_iter()
        // Built-ins whose runtime the node cannot run are filtered from the catalog (registry drift).
        .filter(|d| runnable(&d.runtime))
        .collect();

    let custom = list_definitions(&node.store, ws, false)
        .await
        .map_err(|_| ToolError::Denied)?;
    out.extend(custom);

    Ok(out)
}
