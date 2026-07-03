//! `agent.runtimes` — the read surface for the runtime picker (external-agent sub-scope #5, the
//! run-lifecycle "read surface"). Lists the runtimes/profiles THIS node has configured, so the
//! channel command palette can render a real runtime dropdown (the `x-lb:{widget:"runtime"}` arg)
//! instead of a typed `@id`. Read-only, list-only, workspace-scoped.
//!
//! Shape: `{ "default": <id>, "runtimes": [<sorted ids>], "workspace_default": <pick>|null }`.
//! `default` + `runtimes` are the node registry (a boot-time config map). `workspace_default` is the
//! WORKSPACE's active pick — `{ "runtime": <id>, "label": <human label> }` when the workspace has set
//! `agent.config.default_runtime`, else `null`. No health/version per profile — this is all the picker
//! needs; a richer per-profile shape is a later addition, not this slice.
//!
//! WHY `workspace_default` is here (and additive): the composer runtime dropdown renders where the
//! Settings queries (`agent.config.get` / `agent.def.list`) are NOT loaded, so it cannot resolve the
//! active pick's human LABEL on its own. Folding the resolved pick into this ONE read lets the dropdown
//! show "Active — <label>" without a second fetch. It is best-effort: a config-read error resolves to
//! `null` (the verb never fails on it), so the picker degrades to the registry default.
//!
//! Why it CANNOT leak cross-workspace data: `default`/`runtimes` are registry-derived (no store read).
//! `workspace_default` reads THIS `ws`'s `agent.config` (namespace-scoped, the hard wall) and resolves
//! its label from THIS `ws`'s `agent.def.list` — a ws-B caller can only ever see ws-B's pick. The
//! workspace also gates the CALL (`authorize_tool` is workspace-first), keeping the verb ws-scoped.
//!
//! Single responsibility: list the configured runtimes + the workspace's active pick. The invoke path
//! (`invoke_via_runtime`) and the registry itself live elsewhere; this file only reads and shapes.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde_json::{json, Value};

use crate::agent::{agent_def_list, get_agent_config};
use crate::boot::Node;

/// List the node's configured agent runtimes for `ws` as `principal`. Gated by
/// `mcp:agent.runtimes:call` (workspace-first); a caller without it gets an opaque [`ToolError::Denied`]
/// with no id leaked. Returns `{ "default": <default_id>, "runtimes": [<sorted ids>],
/// "workspace_default": <pick>|null }` — a default-only node with no active pick yields exactly
/// `{ "default": "default", "runtimes": ["default"], "workspace_default": null }`.
pub async fn list_runtimes(
    node: &Node,
    principal: &Principal,
    ws: &str,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, "agent.runtimes").map_err(|_| ToolError::Denied)?;
    let registry = node.runtimes();
    Ok(json!({
        "default": registry.default_id(),
        "runtimes": registry.ids(),
        "workspace_default": workspace_default(node, principal, ws).await,
    }))
}

/// Resolve `ws`'s active pick for the picker: `{ "runtime": <id>, "label": <human label> }`, or
/// [`Value::Null`] when the workspace has set no `default_runtime` (or the config read fails —
/// best-effort, the verb must not fail on it). The label is the matching [`AgentDefinition`]'s
/// `.label` (a definition whose `runtime` or `id` equals the pick), falling back to the runtime id
/// string when no definition matches.
async fn workspace_default(node: &Node, principal: &Principal, ws: &str) -> Value {
    // Best-effort: a config read error → no active pick (degrade to the registry default).
    let pick = match get_agent_config(&node.store, ws).await {
        Ok(Some(cfg)) => cfg.default_runtime,
        _ => None,
    };
    let Some(runtime) = pick else {
        return Value::Null;
    };

    // Resolve the human label from the workspace catalog (built-ins ∪ custom): a definition whose
    // `runtime` matches the pick, else one whose `id` matches; fall back to the id string.
    let label = agent_def_list(node, principal, ws)
        .await
        .ok()
        .and_then(|defs| {
            defs.iter()
                .find(|d| d.runtime == runtime)
                .or_else(|| defs.iter().find(|d| d.id == runtime))
                .map(|d| d.label.clone())
        })
        .unwrap_or_else(|| runtime.clone());

    json!({ "runtime": runtime, "label": label })
}
