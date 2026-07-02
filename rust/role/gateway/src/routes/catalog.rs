//! The i18n-catalog HTTP surface — the browser path, mirroring the host MCP verbs 1:1 (i18n-catalogs
//! scope). All three are GATED tenant verbs (a catalog carries workspace overrides); each forwards to
//! `lb_host::*` which re-checks the capability. The workspace + principal come from the token, never
//! the body (§7).
//!
//!   POST /message/render   -> message.render      (body: { key, args?, recipient? })
//!   POST /prefs/catalog    -> prefs.catalog       (body: { locale })
//!   PUT  /message/catalog  -> message.set_catalog (admin; body: { locale, messages })

use std::collections::BTreeMap;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{message_render, message_set_catalog, prefs_catalog, PrefsSvcError};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// The `POST /message/render` body. `args`/`recipient` optional; `recipient != self` needs the
/// fan-out grant (the host enforces it).
#[derive(Debug, Deserialize)]
pub struct RenderBody {
    pub key: String,
    #[serde(default)]
    pub args: Value,
    #[serde(default)]
    pub recipient: Option<String>,
}

/// `POST /message/render` — render a catalog message in the recipient's resolved language.
pub async fn render_message(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<RenderBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let r = message_render(
        &gw.node.store,
        &p,
        p.ws(),
        &body.key,
        &body.args,
        body.recipient.as_deref(),
    )
    .await
    .map_err(svc_status)?;
    Ok(Json(json!({
        "text": r.text,
        "locale_used": r.locale_used,
        "catalog_version": r.catalog_version,
    })))
}

/// The `POST /prefs/catalog` body — the locale whose merged catalog to fetch.
#[derive(Debug, Deserialize)]
pub struct CatalogBody {
    pub locale: String,
}

/// `POST /prefs/catalog` — the merged (override-over-builtin) catalog for the caller's own workspace.
pub async fn get_catalog(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<CatalogBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let view = prefs_catalog(&gw.node.store, &p, p.ws(), &body.locale)
        .await
        .map_err(svc_status)?;
    Ok(Json(json!({
        "locale": view.locale,
        "catalog_version": view.catalog_version,
        "messages": view.messages,
        "has_override": view.has_override,
    })))
}

/// The `PUT /message/catalog` body — a sparse override patch (flat key→MF1) for a locale.
#[derive(Debug, Deserialize)]
pub struct SetCatalogBody {
    pub locale: String,
    #[serde(default)]
    pub messages: BTreeMap<String, String>,
}

/// `PUT /message/catalog` — merge a workspace override (admin-gated by the host), then the host
/// publishes the "catalog changed" hint.
pub async fn set_catalog(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SetCatalogBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    message_set_catalog(
        &gw.node.store,
        &gw.node.bus,
        &p,
        p.ws(),
        &body.locale,
        body.messages,
    )
    .await
    .map_err(svc_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Map a prefs service error to an HTTP status — a denial is an opaque 403 (no existence signal); a
/// bad input (e.g. a catalog-lint failure) is a 400; a store failure stays opaque (403).
fn svc_status(e: PrefsSvcError) -> (StatusCode, String) {
    match e {
        PrefsSvcError::Denied => (StatusCode::FORBIDDEN, "denied".into()),
        PrefsSvcError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        PrefsSvcError::Store(_) => (StatusCode::FORBIDDEN, "denied".into()),
    }
}
