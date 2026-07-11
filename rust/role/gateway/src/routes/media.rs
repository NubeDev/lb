//! Media HTTP routes — the chunk upload (`PUT /media/{id}/chunk/{n}`) and serve
//! (`GET /media/{id}`) routes (media scope). These carry raw bytes over HTTP, not MCP payloads.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::session::authenticate;
use crate::state::Gateway;

/// `PUT /media/{id}/chunk/{n}` — upload a chunk (raw body). Idempotent (re-PUT upserts).
/// Authenticated; the `media.upload` cap is checked at the `upload_begin`/`commit` MCP verbs.
pub async fn put_media_chunk(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((id, n)): Path<(String, u32)>,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::chunk_write(&gw.node.store, p.ws(), &id, n, &body)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "ok": true, "n": n })))
}

/// `GET /media/{id}?variant=thumb` — serve media bytes (original or variant). Capability-checked
/// (`store:media/{id}:read`), ETag/if-none-match, correct mime.
pub async fn get_media(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<MediaQuery>,
) -> impl IntoResponse {
    let p = match authenticate(&gw, &headers).await {
        Ok(p) => p,
        Err(e) => return e.into_response().into_response(),
    };
    match lb_host::media_serve(&gw.node.store, &p, p.ws(), &id, q.variant.as_deref()).await {
        Ok(served) => {
            // ETag / If-None-Match
            if let Some(etag) = headers.get(header::IF_NONE_MATCH) {
                if etag.as_bytes() == served.etag.as_bytes() {
                    return StatusCode::NOT_MODIFIED.into_response();
                }
            }
            let ctype = HeaderValue::from_str(&served.mime)
                .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
            let etag = HeaderValue::from_str(&served.etag)
                .unwrap_or_else(|_| HeaderValue::from_static("\"\""));
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, ctype),
                    (header::ETAG, etag),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("private, max-age=3600"),
                    ),
                ],
                served.bytes,
            )
                .into_response()
        }
        Err(e) => {
            let code = match &e {
                lb_host::MediaError::Denied => StatusCode::FORBIDDEN,
                lb_host::MediaError::NotFound | lb_host::MediaError::NotReady => {
                    StatusCode::NOT_FOUND
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (code, e.to_string()).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct MediaQuery {
    #[serde(default)]
    pub variant: Option<String>,
}
