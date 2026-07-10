//! Binary-asset routes — the browser's `assets.*` binary surface over the gateway (reports scope,
//! "Binary-asset wiring"). The shipped `/docs` routes serve document *records*; this adds the raw
//! byte PUT/GET the report builder needs: report image-blocks and brand logos. Each route
//! authenticates first, then runs the host's three gates server-side (`store:asset/{id}:{read|write}`).
//!
//! `PUT` accepts a base64 body (JSON is the transport; the gateway decodes to raw `Vec<u8>`). `GET`
//! returns the raw bytes with the asset's stored `mime` (the `ext_ui` bytes tuple + authenticate).

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use lb_host::AssetError;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `POST /assets` body — a base64-encoded binary asset. `id` is workspace-unique (re-`put` upserts).
#[derive(Debug, Deserialize)]
pub struct PutAsset {
    pub id: String,
    pub mime: String,
    /// The raw payload, base64-encoded (JSON transport).
    pub bytes: String,
}

/// `POST /assets` — store a binary asset (UPSERT on `id`). Owner is the token's principal. Gated
/// `store:asset/{id}:write`.
pub async fn put_asset(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<PutAsset>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let bytes = B64
        .decode(body.bytes.as_bytes())
        .map_err(|_| (StatusCode::BAD_REQUEST, "bad base64 asset".to_string()))?;
    lb_host::put_asset(
        &gw.node.store,
        &p,
        p.ws(),
        &body.id,
        &body.mime,
        bytes,
        gw.now(),
    )
    .await
    .map_err(asset_status)?;
    Ok(Json(json!({ "id": body.id })))
}

/// `GET /assets/{id}` — the raw bytes of a binary asset, served with its stored `mime`. Gated
/// `store:asset/{id}:read`. The `ext_ui` bytes tuple + authenticate.
pub async fn get_asset_bin(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let p = match authenticate(&gw, &headers).await {
        Ok(p) => p,
        Err(e) => return e.into_response().into_response(),
    };
    match lb_host::get_asset(&gw.node.store, &p, p.ws(), &id).await {
        Ok(asset) => {
            let ctype = HeaderValue::from_str(&asset.mime)
                .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
            (StatusCode::OK, [(header::CONTENT_TYPE, ctype)], asset.bytes).into_response()
        }
        Err(e) => asset_status(e).into_response(),
    }
}

/// Map an asset gate outcome onto an HTTP status. `NotFound` is `404`; everything else is opaque
/// `403` (matches the existing `/docs` asset routes).
fn asset_status(e: AssetError) -> (StatusCode, String) {
    match e {
        AssetError::NotFound => (StatusCode::NOT_FOUND, e.to_string()),
        other => (StatusCode::FORBIDDEN, other.to_string()),
    }
}
