//! `PATCH /uploads/{sink}/{id}` with `Content-Range` — the streaming half, and the whole reason the
//! seam exists.
//!
//! **Chunks are forwarded as they arrive.** The body is read as a stream and handed to `append` in
//! bounded pieces; lb never collects a request body and never writes an artifact to its own disk.
//! Peak memory is one chunk ([`UPLOAD_CHUNK_BYTES`], 64 KiB), independent of artifact size. Getting
//! this wrong reintroduces the buffer the seam exists to remove, invisibly — so read the loop below
//! with that in mind: there is no `Vec` accumulating the body, no `to_bytes`, no temp file.
//!
//! **Backpressure, not read-ahead.** `append` is `await`ed per chunk before the next frame is pulled
//! from the stream. A client that sends faster than the sink drains is throttled by TCP flow control
//! rather than by lb's heap.
//!
//! **Offsets are the sink's truth.** A range that does not begin at the sink's current offset is a
//! `409` carrying the correct offset (see `gate::status`), so a client that lost track resumes
//! without guessing.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::Json;
use bytes::Bytes;
use futures::StreamExt;
use lb_host::{UploadError, UPLOAD_CHUNK_BYTES};

use super::{content_range, gate};
use crate::state::Gateway;

/// `PATCH /uploads/{sink}/{id}` → `200 {id, offset}`.
pub async fn append_upload(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((sink, id)): Path<(String, String)>,
    body: Body,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let (s, _p) = gate::resolve(&gw, &headers, &sink).await?;

    let range = headers
        .get(header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(content_range::parse)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "PATCH needs a well-formed `Content-Range: bytes {first}-{last}/{total}`".into(),
            )
        })?;

    // The route's own half of "bounded by config, never unlimited": the range's END is checked
    // against the sink's ceiling BEFORE any byte is read, so an oversized stream is cut off at the
    // header rather than after it has spent the disk. The sink enforces its own running total too —
    // two independent checks, because this one can be lied to by a client that under-declares.
    let limit = s.max_upload_bytes();
    if range.last >= limit {
        return Err(gate::status(&UploadError::TooLarge { limit }));
    }

    let mut offset = range.first;
    let mut written = 0u64;
    let mut stream = body.into_data_stream();

    while let Some(frame) = stream.next().await {
        let mut frame: Bytes = frame.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("upload stream ended early: {e}"),
            )
        })?;
        // A client (or a proxy) may hand us a frame larger than one chunk. Split it so the sink
        // always sees bounded pieces and the promise "peak memory is one chunk" holds regardless of
        // who is on the other end. `split_to` is a refcount bump, not a copy.
        while !frame.is_empty() {
            let take = frame.len().min(UPLOAD_CHUNK_BYTES);
            let chunk = frame.split_to(take);
            written += chunk.len() as u64;
            // A stream that exceeds its own declared range is cut off mid-append — the client's
            // framing is the contract it asked to be held to.
            if written > range.len() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!(
                        "stream exceeded its declared Content-Range ({} bytes)",
                        range.len()
                    ),
                ));
            }
            offset = s
                .append(&id, offset, chunk)
                .await
                .map_err(|e| gate::status(&e))?;
        }
    }

    Ok(Json(serde_json::json!({ "id": id, "offset": offset })))
}
