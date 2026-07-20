//! Undo routes — the shell's undo/redo affordance over the gateway (undo-exposure scope). Mirrors
//! the `flows.rs` typed-route precedent: each route maps 1:1 onto a shipped host verb and re-checks
//! its `mcp:<verb>:call` capability server-side via `lb_host::call_tool` (the same MCP chokepoint).
//! The workspace + principal come from the **token**, never the body (§7), so a session can only
//! ever touch its own workspace's journal — and its own stack, unless it holds `mcp:undo.any:call`
//! (the host verb re-gates a foreign `actor`).
//!
//! The verbs' UI-shaped outcomes pass through **typed, not stringly**: an `undo`/`redo` that cannot
//! apply right now returns `200 {ok:false, reason:"stale"|"not_undoable"|"empty", …}` — a normal,
//! explainable result the shell renders (the stale toast, the greyed row) — while a true
//! authorization failure stays an opaque `403`.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_mcp::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `POST /undo` / `POST /redo` body — optional finer stack key (editor-style `surface` undo);
/// absent = the caller's default per-(ws, actor) stack. `actor` (another principal's stack) is
/// deliberately NOT accepted here: the v1 shell never normalizes cross-actor undo (scope "Risks");
/// an admin holding `undo.any` uses MCP.
#[derive(Debug, Default, Deserialize)]
pub struct UndoBody {
    #[serde(default)]
    pub surface: Option<String>,
}

/// `POST /undo` — reverse the caller's newest undoable step. Gated `mcp:undo:call` (plus the host's
/// no-escalation check on the original tool's cap). Refusals come back typed (`ok:false`).
pub async fn post_undo(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    body: Option<Json<UndoBody>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(&gw, &p, "undo", &input(body)).await
}

/// `POST /redo` — re-apply the caller's newest redoable step. Gated `mcp:redo:call`.
pub async fn post_redo(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    body: Option<Json<UndoBody>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(&gw, &p, "redo", &input(body)).await
}

/// `GET /undo/history` — the caller's own stack, newest-first (`{items}` — undoable, greyed
/// irreversible, and already-undone redoable rows). Gated `mcp:history.list:call` (rides the viewer
/// `mcp:*.list:call` read wildcard — seeing history you cannot act on is correct).
pub async fn get_undo_history(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(&gw, &p, "history.list", &json!({})).await
}

/// `GET /undo/history/{seq}/compensations` — the compensating tool a non-undoable step offers
/// (`{compensation_tool}` — `null` for reversible/plain-irreversible steps). Gated
/// `mcp:history.compensations:call` (member tier). A pruned/unknown seq is the typed "no such
/// journal step" error, mapped to a `500`-class extension error — never a panic.
pub async fn get_undo_compensations(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(seq): Path<u64>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(&gw, &p, "history.compensations", &json!({ "step": seq })).await
}

/// The `undo`/`redo` MCP input from an optional body (absent body = the default stack).
fn input(body: Option<Json<UndoBody>>) -> Value {
    match body.and_then(|Json(b)| b.surface) {
        Some(surface) => json!({ "surface": surface }),
        None => json!({}),
    }
}

/// Forward one undo-journal MCP call through the host (re-checking the cap), returning its JSON
/// output — including the typed `ok:false` refusal shapes, passed through as data.
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

/// Map an MCP gate outcome onto HTTP. `Denied` → opaque `403` (no existence signal); `BadInput` →
/// `400` verbatim; a store/journal fault → `500`. The surfaced refusals never reach here — they are
/// `200 {ok:false}` outcomes by design.
fn status(e: ToolError) -> (StatusCode, String) {
    match e {
        ToolError::Denied => (StatusCode::FORBIDDEN, "not permitted".into()),
        ToolError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        ToolError::NotFound => (StatusCode::NOT_FOUND, "no such step".into()),
        ToolError::Extension(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        // Routed-dispatch failures (#81) are not expected on this route — these verbs are
        // host-native and always local, so there is no node to address. Mapped to 500 rather than
        // silently swallowed: if one ever appears here it is a real bug in verb routing, and it
        // should be loud enough to notice.
        e @ (ToolError::Ambiguous { .. }
        | ToolError::NodeUnreachable { .. }
        | ToolError::NodeTooOld { .. }) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}
