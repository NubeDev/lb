//! `resolve_active_definition` — the ONE shared answer to "which agent *definition* is active" for a
//! workspace (active-agent-wiring scope, Slice 2). Promoted from the copy inlined in `defs/test.rs`
//! (`resolve_target`) so the UI badge, `agent.def.test`, rules, and the per-workspace model resolver
//! all agree on the same definition — a single seam, not four re-derivations that can drift.
//!
//! **Precedence (decided):**
//!   1. an **explicit** `id` → that definition (`agent_def_get`, which re-runs its OWN member gate +
//!      built-in/custom namespace split — the wall is inherited, never widened);
//!   2. else the workspace's **`agent.config.active_definition`** id, if set (the first-class pick the
//!      catalog writes) → `agent_def_get(id)`;
//!   3. else the workspace's **`agent.config.default_runtime`** as an id → `agent_def_get(id)`;
//!   4. else the first `agent_def_list` entry whose `runtime` matches that stored `default_runtime`.
//!
//! When none resolve, `BadInput` ("nothing active") — a clear signal, never a panic. This is the
//! definition twin of [`resolve_effective_runtime`](super::resolve_default) (which answers "which
//! *runtime* id") — same shape, one level up (the definition carries the `model_endpoint` a runtime id
//! alone does not).

use lb_auth::Principal;
use lb_mcp::ToolError;

use super::config::get_agent_config;
use super::defs::{agent_def_get, agent_def_list, AgentDefinition};
use crate::boot::Node;

/// Resolve the active definition for `ws` under `caller`: the explicit `id`, else the workspace's
/// `active_definition` pick, else its `default_runtime` (as an id, then as a runtime match). Every
/// lookup re-runs `agent_def_get`'s member gate + namespace split, so the caller can never reach a
/// definition it could not `get`. Returns `BadInput` when nothing is active.
pub async fn resolve_active_definition(
    node: &Node,
    caller: &Principal,
    ws: &str,
    id: Option<&str>,
) -> Result<AgentDefinition, ToolError> {
    // (1) Explicit id → that definition.
    if let Some(id) = id {
        return agent_def_get(node, caller, ws, id).await;
    }

    // Read the workspace pick (best-effort: a store read error is treated as "unset", never a panic).
    let cfg = get_agent_config(&node.store, ws)
        .await
        .map_err(|_| ToolError::Denied)?;

    // (2) The first-class `active_definition` pick, if the catalog wrote one and it still resolves.
    // A dangling id (definition since deleted) falls through to the runtime path below, not an error.
    if let Some(cfg) = cfg.as_ref() {
        if let Some(active) = cfg.active_definition.as_deref().filter(|s| !s.is_empty()) {
            if let Ok(def) = agent_def_get(node, caller, ws, active).await {
                return Ok(def);
            }
        }
    }

    // (3) / (4) Fall back to the stored `default_runtime`: try it as an id, then as a runtime to match.
    let runtime = cfg.and_then(|c| c.default_runtime).ok_or_else(|| {
        ToolError::BadInput("no id given and no active agent is configured".into())
    })?;
    if let Ok(def) = agent_def_get(node, caller, ws, &runtime).await {
        return Ok(def);
    }
    let defs = agent_def_list(node, caller, ws).await?;
    defs.into_iter()
        .find(|d| d.runtime == runtime)
        .ok_or_else(|| ToolError::BadInput("no catalog definition binds the active runtime".into()))
}
