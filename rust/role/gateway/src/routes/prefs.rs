//! The prefs + formatting HTTP surface — the browser path, mirroring the host MCP verbs 1:1 (prefs
//! scope). Gated tenant verbs (`prefs.*`) forward to `lb_host::prefs_*` (which re-checks the
//! capability); the grant-free utility tier (`format.*`/`convert.unit`) forwards to
//! `lb_host::call_format_tool` — authenticated for identity, but needing no capability (pure math,
//! no tenant data). The workspace + caps come from the token, never the body.
//!
//!   GET  /prefs                 -> prefs.get
//!   PUT  /prefs                 -> prefs.set            (body: a Prefs patch + optional `_clear`)
//!   POST /prefs/resolve         -> prefs.resolve        (body: { override?: Prefs })
//!   PUT  /prefs/default         -> prefs.set_default    (admin; body: a Prefs patch + `_clear`)
//!   POST /format/datetime       -> format.datetime
//!   POST /format/number         -> format.number
//!   POST /format/quantity       -> format.quantity
//!   POST /convert/unit          -> convert.unit
//!
//! On the two write routes the body IS the patch (no envelope), so the axes-to-clear list rides an
//! underscore-reserved `_clear` key alongside the axis fields rather than wrapping the body in a new
//! shape — the pre-existing clients that send a bare patch keep working untouched.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{prefs_get, prefs_resolve, prefs_set, prefs_set_default, PrefsSvcError};
use lb_mcp::ToolError;
use lb_prefs::{Prefs, PrefsAxis};
use serde::Deserialize;
use serde_json::Value;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /prefs` — the caller's own stored, nullable prefs.
pub async fn get_prefs(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let prefs = prefs_get(&gw.node.store, &p, p.ws())
        .await
        .map_err(svc_status)?;
    Ok(Json(serde_json::json!({ "prefs": prefs })))
}

/// `PUT /prefs` — merge a patch into the caller's own record, clearing any `_clear` axes.
pub async fn set_prefs(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let (patch, clear) = split_write_body(body)?;
    prefs_set(&gw.node.store, &p, p.ws(), &patch, &clear)
        .await
        .map_err(svc_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// The `POST /prefs/resolve` body — an optional self-scoped request override (preview), never written.
#[derive(Debug, Default, Deserialize)]
pub struct ResolveBody {
    #[serde(default)]
    pub r#override: Option<Prefs>,
}

/// `POST /prefs/resolve` — fold the chain for the caller (with an optional override).
pub async fn resolve_prefs(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<ResolveBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let resolved = prefs_resolve(&gw.node.store, &p, p.ws(), body.r#override)
        .await
        .map_err(svc_status)?;
    Ok(Json(serde_json::json!({ "resolved": resolved })))
}

/// `PUT /prefs/default` — set the workspace-default prefs (admin-gated by the host).
pub async fn set_default_prefs(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let (patch, clear) = split_write_body(body)?;
    prefs_set_default(&gw.node.store, &p, p.ws(), &patch, &clear)
        .await
        .map_err(svc_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Split a write body into the `Prefs` patch and the `_clear` axis list. `_clear` is lifted out
/// before deserializing the patch, so it never reaches `Prefs` as an unknown field — and can never be
/// mistaken for a stored axis. Shared by both write routes.
fn split_write_body(mut body: Value) -> Result<(Prefs, Vec<PrefsAxis>), (StatusCode, String)> {
    let clear_val = body
        .as_object_mut()
        .and_then(|m| m.remove("_clear"))
        .unwrap_or(Value::Null);
    let clear: Vec<PrefsAxis> = match clear_val {
        Value::Null => Vec::new(),
        v => serde_json::from_value(v)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("_clear: {e}")))?,
    };
    let patch: Prefs = serde_json::from_value(body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("patch: {e}")))?;
    Ok((patch, clear))
}

/// `POST /format/datetime` — the grant-free formatter. Authenticated for identity; no cap required.
pub async fn format_datetime(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    util(&gw, &headers, "format.datetime", body).await
}

/// `POST /format/number`.
pub async fn format_number(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    util(&gw, &headers, "format.number", body).await
}

/// `POST /format/quantity` — the chart-formatting bridge.
pub async fn format_quantity(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    util(&gw, &headers, "format.quantity", body).await
}

/// `POST /convert/unit` — raw same-dimension conversion.
pub async fn convert_unit(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    util(&gw, &headers, "convert.unit", body).await
}

/// Shared dispatch for the grant-free utility tier: authenticate (identity only), then call the
/// pure host formatter. A bad input is a 400; there is no capability to deny.
async fn util(
    gw: &Gateway,
    headers: &HeaderMap,
    verb: &str,
    body: Value,
) -> Result<Json<Value>, (StatusCode, String)> {
    authenticate(gw, headers)
        .await
        .map_err(|e| e.into_response())?;
    let out = lb_host::call_format_tool(verb, &body).map_err(tool_status)?;
    Ok(Json(out))
}

/// Map a prefs service error to an HTTP status. A denial is an opaque 403 (no existence signal); a
/// bad input is a 400; a store failure stays opaque (403) at the boundary.
fn svc_status(e: PrefsSvcError) -> (StatusCode, String) {
    match e {
        PrefsSvcError::Denied => (StatusCode::FORBIDDEN, "denied".into()),
        PrefsSvcError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        PrefsSvcError::Store(_) => (StatusCode::FORBIDDEN, "denied".into()),
    }
}

fn tool_status(e: ToolError) -> (StatusCode, String) {
    match e {
        ToolError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        ToolError::NotFound => (StatusCode::NOT_FOUND, "no such tool".into()),
        _ => (StatusCode::FORBIDDEN, "denied".into()),
    }
}
