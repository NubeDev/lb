//! Rules routes — the browser's `rules.*` Playground surface over the gateway (rules-workbench scope,
//! Phase 1). Each route mirrors a shipped `rules.*` MCP verb 1:1 and re-checks the cap server-side by
//! re-entering `lb_host::call_tool` (which authorizes `mcp:rules.<verb>:call`, dispatches the
//! host-native rules service, and wires the AI seam). The workspace + principal come from the **token**
//! (§7), never the body. The deny is opaque (`403 "not permitted"`); author feedback (a parse/cage
//! error, an AI-budget/AI-not-configured message) arrives as `BadInput` and is shown **verbatim**.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_mcp::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `POST /rules/run` body — run an ad-hoc (`body`) or saved (`rule_id`) rule with optional `params`.
#[derive(Debug, Deserialize)]
pub struct RunRule {
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub rule_id: Option<String>,
    #[serde(default)]
    pub params: Value,
}

/// `POST /rules/run` — evaluate a rule, returning `{output, findings, log, ms, ai}`. Gated
/// `mcp:rules.run:call`; a cage/parse fault or an AI-budget/AI-not-configured message is `400` verbatim.
pub async fn run_rule(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<RunRule>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let mut input = json!({});
    if let Some(b) = body.body {
        input["body"] = Value::String(b);
    }
    if let Some(id) = body.rule_id {
        input["rule_id"] = Value::String(id);
    }
    if !body.params.is_null() {
        input["params"] = body.params;
    }
    call(&gw, &p, "rules.run", &input).await
}

/// `POST /rules` body — create/update a saved rule (idempotent UPSERT on `id`).
#[derive(Debug, Deserialize)]
pub struct SaveRule {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub body: String,
    #[serde(default)]
    pub params: Value,
}

/// `POST /rules` — UPSERT a saved rule, returning `{id}`. Gated `mcp:rules.save:call`.
pub async fn save_rule(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SaveRule>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let mut input = json!({ "id": body.id, "body": body.body });
    if let Some(name) = body.name {
        input["name"] = Value::String(name);
    }
    if !body.params.is_null() {
        input["params"] = body.params;
    }
    call(&gw, &p, "rules.save", &input).await
}

/// `GET /rules/{id}` — one saved rule. Gated `mcp:rules.get:call`; an absent/tombstoned rule is `404`.
pub async fn get_rule(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    call(&gw, &p, "rules.get", &json!({ "id": id })).await
}

/// `GET /rules` — the workspace's saved-rule roster, `{rules:[...]}`. Gated `mcp:rules.list:call`.
pub async fn list_rules(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    call(&gw, &p, "rules.list", &json!({})).await
}

/// `DELETE /rules/{id}` — idempotent tombstone. Gated `mcp:rules.delete:call`; `204` on success.
pub async fn delete_rule(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let _ = call(&gw, &p, "rules.delete", &json!({ "id": id })).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Re-enter the host for one `rules.*` verb under the token's principal + workspace, parsing the JSON
/// string result back to a `Value`. The cap re-check happens inside `call_tool`.
async fn call(
    gw: &Gateway,
    p: &lb_auth::Principal,
    tool: &str,
    input: &Value,
) -> Result<Json<Value>, (StatusCode, String)> {
    let out = lb_host::call_tool(&gw.node, p, p.ws(), tool, &input.to_string())
        .await
        .map_err(status)?;
    let value: Value = serde_json::from_str(&out).unwrap_or(Value::String(out));
    Ok(Json(value))
}

/// Map a `ToolError` onto an HTTP status — the cage/deny honesty rule. `Denied` is opaque `403`;
/// `BadInput` is `400` with the message **verbatim** (author feedback: parse/cage error, AI budget
/// exceeded, AI not configured); `NotFound` is `404`; anything else is `500` with its string.
fn status(e: ToolError) -> (StatusCode, String) {
    match e {
        ToolError::Denied => (StatusCode::FORBIDDEN, "not permitted".to_string()),
        ToolError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        ToolError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
        ToolError::Extension(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
    }
}
