//! Media HTTP routes — the chunk upload (`PUT /media/{id}/chunk/{n}`) and serve
//! (`GET /media/{id}`) routes (media scope). These carry raw bytes over HTTP, not MCP payloads.
//! All validation lives at the host layer (`media_chunk_put`, `media_serve`, `plan_serve`);
//! this file only translates host results into status codes + headers.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::session::authenticate;
use crate::state::Gateway;

/// `PUT /media/{id}/chunk/{n}` — upload a chunk (raw body). Idempotent (re-PUT upserts).
/// Capability-checked (`mcp:media.upload:call`) and validated against the upload record
/// (exists, still `Uploading`, `n` in range, body within chunk size) **before any byte is
/// written** — see `lb_host::media_chunk_put`.
pub async fn put_media_chunk(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((id, n)): Path<(String, u32)>,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::media_chunk_put(&gw.node.store, &p, p.ws(), &id, n, &body)
        .await
        .map_err(|e| (media_status(&e), e.to_string()))?;
    Ok(Json(json!({ "ok": true, "n": n })))
}

/// `GET /media/{id}?variant=thumb` — serve media bytes (original or variant). Capability-checked
/// (`store:media/{id}:read`), ETag/If-None-Match (304), single-range `Range` (206/416),
/// correct mime.
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
    let served =
        match lb_host::media_serve(&gw.node.store, &p, p.ws(), &id, q.variant.as_deref()).await {
            Ok(served) => served,
            Err(e) => return (media_status(&e), e.to_string()).into_response(),
        };

    let len = served.bytes.len() as u64;
    let inm = header_str(&headers, header::IF_NONE_MATCH);
    let range = header_str(&headers, header::RANGE);
    let etag =
        HeaderValue::from_str(&served.etag).unwrap_or_else(|_| HeaderValue::from_static("\"\""));
    let ctype = HeaderValue::from_str(&served.mime)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let common = [
        (header::CONTENT_TYPE, ctype),
        (header::ETAG, etag.clone()),
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, max-age=3600"),
        ),
        (header::ACCEPT_RANGES, HeaderValue::from_static("bytes")),
    ];

    match lb_host::plan_serve(len, &served.etag, inm, range) {
        lb_host::ServePlan::NotModified => {
            (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response()
        }
        lb_host::ServePlan::Full => (StatusCode::OK, common, served.bytes).into_response(),
        lb_host::ServePlan::Partial { start, end } => {
            let slice = served.bytes[start as usize..=end as usize].to_vec();
            let cr = HeaderValue::from_str(&format!("bytes {start}-{end}/{len}"))
                .unwrap_or_else(|_| HeaderValue::from_static("bytes */0"));
            (
                StatusCode::PARTIAL_CONTENT,
                common,
                [(header::CONTENT_RANGE, cr)],
                slice,
            )
                .into_response()
        }
        lb_host::ServePlan::Unsatisfiable => {
            let cr = HeaderValue::from_str(&format!("bytes */{len}"))
                .unwrap_or_else(|_| HeaderValue::from_static("bytes */0"));
            (
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(header::CONTENT_RANGE, cr), (header::ETAG, etag)],
            )
                .into_response()
        }
    }
}

/// Map a host media error to an HTTP status.
fn media_status(e: &lb_host::MediaError) -> StatusCode {
    match e {
        lb_host::MediaError::Denied => StatusCode::FORBIDDEN,
        lb_host::MediaError::NotFound | lb_host::MediaError::NotReady => StatusCode::NOT_FOUND,
        lb_host::MediaError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        lb_host::MediaError::BadChecksum
        | lb_host::MediaError::MissingChunks
        | lb_host::MediaError::BadInput(_) => StatusCode::BAD_REQUEST,
        lb_host::MediaError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn header_str<'a>(headers: &'a HeaderMap, name: header::HeaderName) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

#[derive(Debug, Deserialize)]
pub struct MediaQuery {
    #[serde(default)]
    pub variant: Option<String>,
}
