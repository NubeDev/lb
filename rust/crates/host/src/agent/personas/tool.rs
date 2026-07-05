//! The MCP bridge for the `agent.persona.*` surface (persona-model scope). Dispatches the five gated
//! verbs; DTO shaping mirrors the sibling surfaces (`{ personas }` / `{ persona }` / `{ ok }`). Reached
//! from `call_agent_tool` (the `agent.persona.` branch in `agent/tool.rs`). Returns `None` for a verb
//! outside this surface so it composes as a fall-through before `NotFound`.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde_json::{json, Value};

use super::create::agent_persona_create;
use super::delete::agent_persona_delete;
use super::get::agent_persona_get;
use super::list::agent_persona_list;
use super::model::Persona;
use super::resolve::{resolve_effective, resolve_persona};
use super::update::{agent_persona_update, PersonaPatch};
use crate::boot::Node;

/// Dispatch an `agent.persona.*` verb.
pub async fn call_agent_persona_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Option<Result<Value, ToolError>> {
    match qualified_tool {
        "agent.persona.list" => Some(list(node, principal, ws).await),
        "agent.persona.get" => Some(get(node, principal, ws, input).await),
        "agent.persona.resolve" => Some(resolve(node, principal, ws, input).await),
        "agent.persona.create" => Some(create(node, principal, ws, input).await),
        "agent.persona.update" => Some(update(node, principal, ws, input).await),
        "agent.persona.delete" => Some(delete(node, principal, ws, input).await),
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
    let personas = agent_persona_list(node, principal, ws).await?;
    Ok(json!({ "personas": personas }))
}

async fn get(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let persona = agent_persona_get(node, principal, ws, &arg_id(input)?).await?;
    Ok(json!({ "persona": persona }))
}

/// `agent.persona.resolve {id?}` (member) — the **effective** persona for a run: the `extends`-closure
/// union of tools + pinned skills + the (child-wins) identity, plus the optional policy preset +
/// runtime restriction. `id` names a specific persona; absent → the workspace `active_persona` (or
/// `null` when none). Powers the Settings "effective tools" view (the UI intersects the returned
/// `granted_tools` against the caller's live `tools.catalog` to show `persona ∩ agent ∩ caller` with a
/// reason per exclusion). Member-gated by `mcp:agent.persona.resolve:call`; the `get` gate is inherited
/// on the explicit-id path. Returns `{ effective: null }` when no persona applies.
async fn resolve(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, "agent.persona.resolve").map_err(|_| ToolError::Denied)?;
    let id = input.get("id").and_then(Value::as_str);
    match resolve_persona(node, principal, ws, id).await? {
        Some(persona) => {
            let effective = resolve_effective(node, principal, ws, &persona).await?;
            Ok(json!({ "effective": effective }))
        }
        None => Ok(json!({ "effective": Value::Null })),
    }
}

async fn create(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let persona: Persona = serde_json::from_value(input.clone())
        .map_err(|e| ToolError::BadInput(format!("persona: {e}")))?;
    agent_persona_create(node, principal, ws, &persona).await?;
    Ok(json!({ "ok": true }))
}

async fn update(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let id = arg_id(input)?;
    let patch: PersonaPatch = match input.get("patch") {
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| ToolError::BadInput(format!("patch: {e}")))?,
        None => return Err(ToolError::BadInput("missing arg: patch".into())),
    };
    agent_persona_update(node, principal, ws, &id, patch).await?;
    Ok(json!({ "ok": true }))
}

async fn delete(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    agent_persona_delete(node, principal, ws, &arg_id(input)?).await?;
    Ok(json!({ "ok": true }))
}
