//! Admin API-key routes — the `apikey.*` management surface over the gateway (api-keys scope).
//! Mirror `lb_host::apikey_*` 1:1; authenticated by the session token (the workspace + caps come
//! from the token, never the request — the hard wall §7); gated server-side on `mcp:apikey.manage:
//! call` inside each host verb. The UI cap-gate is convenience only — these routes are the truth (a
//! forged call by a non-admin is denied here).
//!
//! `create`/`rotate` return the raw secret bearer string **once**; `list`/`get` never carry the hash
//! or secret. The pepper is the gateway's node secret (`LB_APIKEY_PEPPER`).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{ApiKeyFull, ApiKeyView};
use serde::{Deserialize, Serialize};

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /admin/apikeys` — every key in the session's workspace (credential-free views).
pub async fn list_apikeys(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<ApiKeyView>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let keys = lb_host::apikey_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(forbid)?;
    Ok(Json(keys))
}

/// The `POST /admin/apikeys` body.
#[derive(Debug, Deserialize)]
pub struct CreateApiKey {
    pub label: String,
    /// `appliance | cli | api | agent` (labelling only).
    #[serde(default)]
    pub kind: Option<String>,
    /// A role name (`apikey-read` / `apikey-write` / custom), or empty for caps-only.
    #[serde(default)]
    pub role: Option<String>,
    /// Additional narrowing caps granted directly to the key.
    #[serde(default)]
    pub caps: Vec<String>,
    /// Unix-secs expiry (`0` / omitted = never).
    #[serde(default)]
    pub expires_at: Option<u64>,
}

/// The `POST /admin/apikeys` reply: the one-time bearer string carrying the raw secret. Shown once.
#[derive(Debug, Serialize)]
pub struct CreatedApiKey {
    pub key: String,
}

/// `POST /admin/apikeys` — mint a key; returns the one-time bearer string (the ONLY egress of the
/// secret). A privilege-escalation refusal (effective caps exceed the creator's) is a `400`.
pub async fn create_apikey(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<CreateApiKey>,
) -> Result<Json<CreatedApiKey>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let role = body.role.as_deref().unwrap_or("apikey-read");
    let key = lb_host::apikey_create(
        &gw.node.store,
        &p,
        p.ws(),
        &gw.pepper,
        &body.label,
        body.kind.as_deref().unwrap_or("api"),
        role,
        &body.caps,
        body.expires_at.unwrap_or(0),
        gw.now,
    )
    .await
    .map_err(apikey_status)?;
    Ok(Json(CreatedApiKey { key }))
}

/// `GET /admin/apikeys/{id}` — one key's full view incl. its resolved cap set (no hash/secret).
pub async fn get_apikey(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiKeyFull>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let full = lb_host::apikey_get(&gw.node.store, &p, p.ws(), &id)
        .await
        .map_err(apikey_status)?;
    Ok(Json(full))
}

/// `POST /admin/apikeys/{id}/revoke` — tombstone + cache-bust + grant-revoke (instant local revoke).
pub async fn revoke_apikey(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::apikey_revoke(&gw.node.store, &gw.node.apikeys, &p, p.ws(), &id)
        .await
        .map_err(apikey_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /admin/apikeys/{id}/rotate` — new secret, old dead instantly; returns the one-time new
/// bearer string.
pub async fn rotate_apikey(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<CreatedApiKey>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let key = lb_host::apikey_rotate(
        &gw.node.store,
        &gw.node.apikeys,
        &p,
        p.ws(),
        &id,
        &gw.pepper,
    )
    .await
    .map_err(apikey_status)?;
    Ok(Json(CreatedApiKey { key }))
}

/// Map an apikey service error to an HTTP status. `Denied` is an opaque `403`; a no-widening refusal
/// or bad input is a `400` (the admin console needs the reason); auth failures never reach here (the
/// session is already authenticated); store/other errors stay opaque (`403`) at the boundary.
fn apikey_status(e: lb_host::ApiKeyError) -> (StatusCode, String) {
    use lb_host::ApiKeyError::*;
    match e {
        Denied => (StatusCode::FORBIDDEN, "denied".into()),
        Widen(c) => (
            StatusCode::BAD_REQUEST,
            format!("cannot grant a cap the creator lacks: {c}"),
        ),
        BadInput(m) => (StatusCode::BAD_REQUEST, m),
        NotFound => (StatusCode::NOT_FOUND, "not found".into()),
        Store(_) => (StatusCode::FORBIDDEN, "denied".into()),
        // Revoked/Expired/Invalid are auth-path outcomes; on a management verb they map to a 4xx.
        Revoked | Expired | Invalid => (StatusCode::BAD_REQUEST, e.to_string()),
    }
}

fn forbid(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, e.to_string())
}
