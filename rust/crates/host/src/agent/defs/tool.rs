//! The MCP bridge for the `agent.def.*` surface (agent-catalog scope). Dispatches the five gated
//! verbs; DTO shaping mirrors the sibling surfaces (`{ definitions }` / `{ definition }` / `{ ok }`).
//! Reached from `call_agent_tool` (the `agent.def.` branch in `agent/tool.rs`). Returns `None` for a
//! verb outside this surface so it composes as a fall-through before `NotFound`.

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use super::create::agent_def_create;
use super::delete::agent_def_delete;
use super::get::agent_def_get;
use super::list::agent_def_list;
use super::model::AgentDefinition;
use super::update::{agent_def_update, DefinitionPatch};
use crate::boot::Node;

/// Dispatch an `agent.def.*` verb.
pub async fn call_agent_catalog_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Option<Result<Value, ToolError>> {
    match qualified_tool {
        "agent.def.list" => Some(list(node, principal, ws).await),
        "agent.def.get" => Some(get(node, principal, ws, input).await),
        "agent.def.create" => Some(create(node, principal, ws, input).await),
        "agent.def.update" => Some(update(node, principal, ws, input).await),
        "agent.def.delete" => Some(delete(node, principal, ws, input).await),
        _ => None,
    }
}

fn arg_id(input: &Value) -> Result<String, ToolError> {
    input
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::BadInput("missing arg: id".into()))
}

async fn list(node: &Node, principal: &Principal, ws: &str) -> Result<Value, ToolError> {
    let definitions = agent_def_list(node, principal, ws).await?;
    Ok(json!({ "definitions": definitions }))
}

async fn get(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let definition = agent_def_get(node, principal, ws, &arg_id(input)?).await?;
    Ok(json!({ "definition": definition }))
}

async fn create(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let def: AgentDefinition = serde_json::from_value(input.clone())
        .map_err(|e| ToolError::BadInput(format!("definition: {e}")))?;
    agent_def_create(node, principal, ws, &def).await?;
    Ok(json!({ "ok": true }))
}

async fn update(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let id = arg_id(input)?;
    let patch: DefinitionPatch = match input.get("patch") {
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| ToolError::BadInput(format!("patch: {e}")))?,
        None => return Err(ToolError::BadInput("missing arg: patch".into())),
    };
    agent_def_update(node, principal, ws, &id, patch).await?;
    Ok(json!({ "ok": true }))
}

async fn delete(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    agent_def_delete(node, principal, ws, &arg_id(input)?).await?;
    Ok(json!({ "ok": true }))
}
