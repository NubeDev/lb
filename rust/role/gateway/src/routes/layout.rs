//! Layout routes — the browser's `layout.*` surface over the gateway (data-studio scope v2). Each
//! route mirrors an `lb_host::layout_*` verb 1:1 and re-runs the host's gates server-side
//! (workspace-first → `mcp:layout.<verb>:call`). The workspace + owner come from the **token**, never
//! the body (§7) — the layout record is keyed to the authenticated principal's `sub`, so a caller can
//! never read or write another user's layout. The UI cap-gate is convenience only; this is the
//! boundary.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::LayoutError;
use serde::Deserialize;
use serde_json::Value;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /layout/{surface}` — read the caller's own layout for a surface. Gated `layout.get`;
/// member-level. Absent → a default record (empty `model`).
pub async fn get_layout(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(surface): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let l = lb_host::layout_get(&gw.node.store, &p, p.ws(), &surface)
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(l).unwrap_or(Value::Null)))
}

/// `PUT /layout/{surface}` body — the client's opaque layout JSON.
#[derive(Debug, Deserialize)]
pub struct SetLayout {
    #[serde(default)]
    pub model: Value,
}

/// `PUT /layout/{surface}` — upsert the caller's own layout. Keyed to the token `sub`. Gated
/// `layout.set`; member-level. Returns the stored record.
pub async fn set_layout(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(surface): Path<String>,
    Json(body): Json<SetLayout>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let l = lb_host::layout_set(&gw.node.store, &p, p.ws(), &surface, body.model, gw.now())
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(l).unwrap_or(Value::Null)))
}

/// Map a layout gate outcome onto an HTTP status. `Denied` is `403` (opaque); `BadInput` `400`; a
/// store fault is `403`-opaque like the other gateway routes.
fn status(e: LayoutError) -> (StatusCode, String) {
    match e {
        LayoutError::Denied => (StatusCode::FORBIDDEN, e.to_string()),
        LayoutError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        LayoutError::Store(s) => (StatusCode::FORBIDDEN, s.to_string()),
    }
}
