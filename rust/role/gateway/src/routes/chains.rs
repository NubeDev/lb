//! Chain routes — the browser's `chains.*` surface over the gateway (rules-workbench scope, Phase 2).
//! Each route mirrors a shipped `chains.*` host verb 1:1 and re-checks the `mcp:chains.<verb>:call`
//! capability server-side via `lb_host::call_tool` (the same MCP chokepoint `routes/mcp.rs` uses). The
//! workspace + principal come from the **token**, never the body (§7) — so a chain is workspace-walled
//! and un-spoofable. The UI cap-gate is convenience only; this is the boundary.
//!
//! The host classifies a failure as a `lb_mcp::ToolError`; [`status`] maps it onto HTTP. A cyclic /
//! invalid DAG arrives as `BadInput(msg)` → `400` with the validation message VERBATIM — the source of
//! the canvas's inline edge error.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_mcp::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /chains` — the workspace's chains (summaries via the host's `chains.list`). Gated
/// `chains.list`.
pub async fn list_chains(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(&gw, &p, "chains.list", &json!({})).await
}

/// `GET /chains/{id}` — one chain (its DAG). Gated `chains.get`; an absent/tombstoned chain → `404`.
pub async fn get_chain(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(&gw, &p, "chains.get", &json!({ "id": id })).await
}

/// `POST /chains` — create/update a chain (DAG-validated UPSERT). Body is the full `Chain` minus the
/// workspace (the host sets it from the token). Gated `chains.save`; an invalid DAG → `400` with the
/// host's validation message (the canvas inline error). Returns `{id}`.
pub async fn save_chain(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(mut body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    // The `Chain` shape carries `workspace`; set it from the token so the body never spoofs it (§7).
    if let Some(obj) = body.as_object_mut() {
        obj.insert("workspace".into(), Value::String(p.ws().to_string()));
    }
    call(&gw, &p, "chains.save", &body).await
}

/// `DELETE /chains/{id}` — idempotent tombstone. Gated `chains.delete`. `204` on success.
pub async fn delete_chain(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let _ = call(&gw, &p, "chains.delete", &json!({ "id": id })).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /chains/{id}/run` body — optional run params.
#[derive(Debug, Default, Deserialize)]
pub struct RunChain {
    #[serde(default)]
    pub params: Value,
}

/// `POST /chains/{id}/run` — start a chain run (a durable job). Gated `chains.run`. Returns
/// `{run_id}` the canvas then polls.
pub async fn run_chain(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<RunChain>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let input = json!({ "chain_id": id, "params": body.params, "ts": gw.now });
    call(&gw, &p, "chains.run", &input).await
}

/// `GET /chains/{id}/runs/{run_id}` — the per-step run snapshot the canvas colours from (gated
/// `chains.runs.get`). The records are the source of truth: a late open rebuilds the same colours.
pub async fn get_chain_run(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(
        &gw,
        &p,
        "chains.runs.get",
        &json!({ "chain_id": id, "run_id": run_id }),
    )
    .await
}

/// Forward one `chains.*` MCP call through the host (re-checking the cap), returning its JSON output.
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

/// Map an MCP gate outcome onto an HTTP status. `Denied` → `403` (opaque "not permitted"); `BadInput`
/// → `400` with the message VERBATIM (the canvas inline DAG error); `NotFound` → `404`; an extension/
/// store fault → `500`.
fn status(e: ToolError) -> (StatusCode, String) {
    match e {
        ToolError::Denied => (StatusCode::FORBIDDEN, "not permitted".into()),
        ToolError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        ToolError::NotFound => (StatusCode::NOT_FOUND, "no such chain".into()),
        ToolError::Extension(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
    }
}
