//! The agent-definition catalog HTTP surface — the browser path, mirroring the host `agent.def.*` MCP
//! verbs 1:1 (agent-catalog scope). Every verb is a GATED tenant verb re-checked host-side; the
//! workspace + caps come from the token, never the body.
//!
//!   GET    /agent/defs         -> agent.def.list    (member)
//!   POST   /agent/defs         -> agent.def.create  (admin; body: an AgentDefinition)
//!   GET    /agent/defs/{id}    -> agent.def.get     (member)
//!   PATCH  /agent/defs/{id}    -> agent.def.update  (admin; body: a DefinitionPatch)
//!   DELETE /agent/defs/{id}    -> agent.def.delete  (admin)
//!   POST   /agent/defs/{id}/test -> agent.def.test  (admin; the context-proving diagnostic)
//!   POST   /agent/defs/test      -> agent.def.test  (admin; the active `agent.config` pick)

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{
    agent_def_create, agent_def_delete, agent_def_get, agent_def_list, agent_def_test,
    agent_def_update, AgentDefinition, DefinitionPatch,
};
use lb_mcp::ToolError;
use serde_json::Value;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /agent/defs` — the catalog (node-runnable built-ins ∪ workspace custom).
pub async fn list_defs(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let definitions = agent_def_list(&gw.node, &p, p.ws())
        .await
        .map_err(tool_status)?;
    Ok(Json(serde_json::json!({ "definitions": definitions })))
}

/// `GET /agent/defs/{id}` — one catalog entry.
pub async fn get_def(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let definition = agent_def_get(&gw.node, &p, p.ws(), &id)
        .await
        .map_err(tool_status)?;
    Ok(Json(serde_json::json!({ "definition": definition })))
}

/// `POST /agent/defs` — create a custom definition (admin-gated by the host).
pub async fn create_def(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(def): Json<AgentDefinition>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    agent_def_create(&gw.node, &p, p.ws(), &def)
        .await
        .map_err(tool_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `PATCH /agent/defs/{id}` — edit a custom definition (admin-gated by the host).
pub async fn update_def(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(patch): Json<DefinitionPatch>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    agent_def_update(&gw.node, &p, p.ws(), &id, patch)
        .await
        .map_err(tool_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /agent/defs/{id}` — remove a custom definition (admin-gated by the host).
pub async fn delete_def(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    agent_def_delete(&gw.node, &p, p.ws(), &id)
        .await
        .map_err(tool_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /agent/defs/{id}/test` — the context-proving diagnostic for one definition (admin-gated by
/// the host). Assembles the caller's real context (system prompt + reachable tools + granted skills)
/// and runs ONE model turn, returning `{ answer, runtime, model, context, provider_configured, ok }`.
pub async fn test_def(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let result = agent_def_test(&gw.node, &p, p.ws(), Some(&id))
        .await
        .map_err(tool_status)?;
    Ok(Json(
        serde_json::to_value(result).unwrap_or(Value::Null),
    ))
}

/// `POST /agent/defs/test` — test the workspace's ACTIVE `agent.config` pick (no id). Same shape.
pub async fn test_active_def(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let result = agent_def_test(&gw.node, &p, p.ws(), None)
        .await
        .map_err(tool_status)?;
    Ok(Json(
        serde_json::to_value(result).unwrap_or(Value::Null),
    ))
}

/// Map a tool error to an HTTP status. A denial is an opaque 403; a reserved/unknown-runtime is 400;
/// an absent id is 404.
fn tool_status(e: ToolError) -> (StatusCode, String) {
    match e {
        ToolError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        ToolError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
        _ => (StatusCode::FORBIDDEN, "denied".into()),
    }
}
