//! The per-workspace agent-config HTTP surface — the browser path, mirroring the host MCP verbs 1:1
//! (agent-config scope). Both are GATED tenant verbs: `agent.config.get` (member) forwards to
//! `lb_host::agent_config_get`, `agent.config.set` (admin) to `lb_host::agent_config_set` (each
//! re-checks the capability). The workspace + caps come from the token, never the body.
//!
//!   GET  /agent/config  -> agent.config.get
//!   PUT  /agent/config  -> agent.config.set   (admin; body: an AgentConfig patch)

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{agent_config_get, agent_config_set, AgentConfig};
use lb_mcp::ToolError;
use serde_json::Value;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /agent/config` — the workspace's stored agent config (`null` when unset).
pub async fn get_agent_config(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let config = agent_config_get(&gw.node, &p, p.ws())
        .await
        .map_err(tool_status)?;
    Ok(Json(serde_json::json!({ "config": config })))
}

/// `PUT /agent/config` — merge a patch into the workspace's agent config (admin-gated by the host).
pub async fn set_agent_config(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(patch): Json<AgentConfig>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    agent_config_set(&gw.node, &p, p.ws(), &patch)
        .await
        .map_err(tool_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Map a tool error to an HTTP status. A denial is an opaque 403 (no existence signal); a bad input
/// (e.g. an unknown runtime id) is a 400.
fn tool_status(e: ToolError) -> (StatusCode, String) {
    match e {
        ToolError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        ToolError::NotFound => (StatusCode::NOT_FOUND, "no such tool".into()),
        _ => (StatusCode::FORBIDDEN, "denied".into()),
    }
}
