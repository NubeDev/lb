//! `POST /viz/query_batch/stream` — the **streamed batch** a dashboard paints progressively from
//! (dashboard-query-acceleration scope, slice 4). The body is the SAME `{panels, now?, cache?}` the
//! `viz.query_batch` verb takes over `/mcp/call`; the response is an NDJSON body — one line per panel,
//! `{"i": <request index>, "result": <the same {frames, rows} | {status, message}>}`, written the moment
//! that panel resolves, in completion order. A closed body with no error line is "every panel
//! answered" (the count equals `panels.len()`).
//!
//! Why a route and not the verb: `/mcp/call` is one JSON answer, so a batch there paints at the speed
//! of its slowest tile. This keeps everything the batch verb bought (one round-trip, one gate check,
//! server-side concurrency, per-panel gateway cache, per-item partial failure) and adds first-paint.
//!
//! Auth is the same bearer session `/mcp/call` uses (`authenticate`); the capability is the same
//! `mcp:viz.query:call` (checked in `lb_host::viz_query_batch_stream`, the aliased batch gate — no
//! new privilege). A caller without it is `403` before any byte of body, exactly as the verb.
//!
//! `X-Accel-Buffering: no` + `Cache-Control: no-cache` so an nginx/edge in front forwards lines as
//! they are written instead of buffering the whole body (which would silently turn this back into the
//! batch verb).

use std::convert::Infallible;

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::StreamExt;
use serde_json::{json, Value};

use super::mcp::tool_error_status;
use crate::session::authenticate;
use crate::state::Gateway;

/// Open the streamed batch. `401` if the session token is missing/bad; `403` if the principal lacks
/// `mcp:viz.query:call`; `400` on a malformed/over-cap batch; otherwise `200` + an NDJSON body.
pub async fn viz_query_batch_stream(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let ws = principal.ws().to_string();

    // Outermost entry (depth 0), same as `/mcp/call` — so the per-panel gateway cache participates.
    let items = lb_host::viz_query_batch_stream(gw.node.clone(), principal, ws, body, 0)
        .map_err(tool_error_status)?;

    let lines = items.map(|item| {
        let mut line = json!({ "i": item.index, "result": item.result }).to_string();
        line.push('\n');
        Ok::<Bytes, Infallible>(Bytes::from(line))
    });

    let mut response = Body::from_stream(lines).into_response();
    let h = response.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-ndjson"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    h.insert("x-accel-buffering", HeaderValue::from_static("no"));
    Ok(response)
}
