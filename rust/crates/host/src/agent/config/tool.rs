//! The MCP bridge for the `agent.config.*` surface (agent-config scope). Dispatches the two gated
//! verbs; the DTO shaping mirrors `prefs.*` (`{ config }` / `{ ok: true }`). Reached from
//! `call_agent_tool` (the `agent.` prefix branch in `tool_call.rs`).

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use super::model::AgentConfig;
use super::verbs::{agent_config_get, agent_config_set};
use crate::boot::Node;

/// Dispatch an `agent.config.*` verb. The patch for `set` is an [`AgentConfig`] JSON object under
/// `patch`. Returns `None` for a verb outside this surface (the caller falls through).
pub async fn call_agent_config_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Option<Result<Value, ToolError>> {
    match qualified_tool {
        "agent.config.get" => Some(get(node, principal, ws).await),
        "agent.config.set" => Some(set(node, principal, ws, input).await),
        _ => None,
    }
}

async fn get(node: &Node, principal: &Principal, ws: &str) -> Result<Value, ToolError> {
    let config = agent_config_get(node, principal, ws).await?;
    Ok(json!({ "config": config }))
}

async fn set(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let patch: AgentConfig = {
        let v = input
            .get("patch")
            .ok_or_else(|| ToolError::BadInput("missing arg: patch".into()))?;
        serde_json::from_value(v.clone()).map_err(|e| ToolError::BadInput(format!("patch: {e}")))?
    };
    agent_config_set(node, principal, ws, &patch).await?;
    Ok(json!({ "ok": true }))
}
