//! `POST /mcp/call` — the **host-mediated bridge endpoint** an extension page/widget reaches platform
//! functionality through (ui-federation scope). It is the universal contract (rule 7) over HTTP: the
//! shell forwards a page's `{tool, args}` here; the gateway authenticates the **session token it holds**
//! (the page never has it), then runs `lb_mcp::call`, which **re-checks the workspace + the
//! `mcp:<tool>:call` capability** before dispatching. A page is therefore exactly as denied as a forged
//! call — the boundary is the host, the bridge is plumbing.
//!
//! The workspace comes from the token (§7), never the body; the body carries only the tool name and its
//! JSON args. An ungranted tool → `403` with no existence signal (the MCP deny contract).

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::session::authenticate;
use crate::state::Gateway;

/// The bridge request: a qualified MCP tool name + its JSON args. No token, no workspace — both come
/// from the verified session, not the page.
#[derive(Debug, Deserialize)]
pub struct McpCall {
    pub tool: String,
    #[serde(default)]
    pub args: Value,
}

/// Forward one bridged MCP tool call. `401` if the session token is missing/bad; `403` if the verified
/// principal lacks `mcp:<tool>:call` (or the tool is unknown — opaque); the tool's JSON output otherwise.
pub async fn mcp_call(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<McpCall>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let input = if body.args.is_null() {
        "{}".to_string()
    } else {
        body.args.to_string()
    };
    let out = lb_host::call_tool(&gw.node, &principal, principal.ws(), &body.tool, &input)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    let value: Value = serde_json::from_str(&out).unwrap_or(Value::String(out));
    Ok(Json(value))
}
