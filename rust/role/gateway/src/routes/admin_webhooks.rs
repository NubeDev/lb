//! Admin webhook routes — the `webhook.*` management surface over the gateway (webhooks scope).
//! Mirror `lb_host::webhook_*` 1:1; authenticated by the session token (the workspace + caps come
//! from the token, never the request — the hard wall §7); gated server-side on `mcp:webhook.
//! manage:call` inside each host verb. The UI cap-gate is convenience only — these routes are the
//! truth (a forged call by a non-admin is denied here).
//!
//! `create`/`rotate` return the raw secret **once** (the `lbk_…` bearer for `bearer` mode, the
//! shared secret for `signature` mode); `list`/`get` never carry the hash / shared-secret / linked
//! apikey id. The pepper is the gateway's node secret (`LB_APIKEY_PEPPER`).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{WebhookAuthMode, WebhookCreateArgs, WebhookView};
use serde::{Deserialize, Serialize};

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /admin/webhooks` — every webhook in the session's workspace (credential-free views).
pub async fn list_webhooks(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<WebhookView>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let views = lb_host::webhook_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(forbid)?;
    Ok(Json(views))
}

/// The `POST /admin/webhooks` body.
#[derive(Debug, Deserialize)]
pub struct CreateWebhook {
    pub name: String,
    /// `"bearer"` or `"signature"` (the two auth modes).
    pub auth_mode: String,
    /// `signature` mode: the header name the caller signs. Default `X-Signature`.
    #[serde(default)]
    pub hmac_header: Option<String>,
}

/// `POST /admin/webhooks` — create a webhook; returns the URL + the one-time secret (the ONLY
/// egress of the raw credential). A privilege-escalation refusal is `400`.
pub async fn create_webhook(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<CreateWebhook>,
) -> Result<Json<lb_host::CreatedWebhook>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let auth_mode = WebhookAuthMode::parse(&body.auth_mode).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            format!(
                "unknown auth_mode '{}' (expected 'bearer' or 'signature')",
                body.auth_mode
            ),
        )
    })?;
    let args = WebhookCreateArgs {
        name: &body.name,
        auth_mode,
        hmac_header: body.hmac_header.as_deref(),
    };
    let created = lb_host::webhook_create(&gw.node.store, &p, p.ws(), &gw.pepper, args, gw.now())
        .await
        .map_err(webhook_status)?;
    Ok(Json(created))
}

/// `GET /admin/webhooks/{id}` — one webhook's credential-free view.
pub async fn get_webhook(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<WebhookView>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let view = lb_host::webhook_get(&gw.node.store, &p, p.ws(), &id)
        .await
        .map_err(webhook_status)?;
    Ok(Json(view))
}

/// `POST /admin/webhooks/{id}/revoke` — tombstone + linked-apikey revoke + cache-bust.
pub async fn revoke_webhook(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::webhook_revoke(&gw.node.store, &gw.node.apikeys, &p, p.ws(), &id)
        .await
        .map_err(webhook_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// The `POST /admin/webhooks/{id}/rotate` reply: the one-time new raw credential.
#[derive(Debug, Serialize)]
pub struct RotatedWebhook {
    pub secret: String,
}

/// `POST /admin/webhooks/{id}/rotate` — replace the credential; old dead instantly. Returns the
/// one-time new raw credential (the `lbk_…` bearer or the new shared secret).
pub async fn rotate_webhook(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<RotatedWebhook>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let secret = lb_host::webhook_rotate(
        &gw.node.store,
        &gw.node.apikeys,
        &p,
        p.ws(),
        &id,
        &gw.pepper,
    )
    .await
    .map_err(webhook_status)?;
    Ok(Json(RotatedWebhook { secret }))
}

/// Map a webhook service error to an HTTP status. `Denied` is an opaque `403`; a no-widening
/// refusal or bad input is a `400` (the admin console needs the reason); `NotFound` is a `404`;
/// `Revoked` is a `409` (you cannot rotate a revoked hook); store/other errors stay opaque (`502`).
fn webhook_status(e: lb_host::WebhookError) -> (StatusCode, String) {
    use lb_host::WebhookError::{BadInput, Denied, Invalid, NotFound, Revoked, Store, Widen};
    match e {
        Denied => (StatusCode::FORBIDDEN, "denied".into()),
        Widen(c) => (
            StatusCode::BAD_REQUEST,
            format!("cannot grant a cap the creator lacks: {c}"),
        ),
        BadInput(m) => (StatusCode::BAD_REQUEST, m),
        NotFound => (StatusCode::NOT_FOUND, "not found".into()),
        Revoked => (
            StatusCode::CONFLICT,
            "webhook is revoked — rotate refused".into(),
        ),
        Invalid => (StatusCode::BAD_REQUEST, "invalid".into()),
        Store(_) => (StatusCode::BAD_GATEWAY, "unavailable".into()),
    }
}

fn forbid(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, e.to_string())
}
