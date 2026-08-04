//! `POST /uploads/{sink}/{id}/complete` and `DELETE /uploads/{sink}/{id}` — the two ways an upload
//! ends.
//!
//! **lb does not verify artifacts.** It carries `digest_hex` and `meta` as opaque values to
//! `complete` and reports the sink's verdict **verbatim**. Verification, signature trust and the
//! content-addressed cache belong to the backend, which already has all three; a second trust chain
//! in lb would be one more thing free to disagree.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::UploadMeta;

use super::gate;
use crate::state::Gateway;

/// `POST /uploads/{sink}/{id}/complete` → the sink's verdict, unmodified.
pub async fn complete_upload(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((sink, id)): Path<(String, String)>,
    body: Option<Json<UploadMeta>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let (s, _p) = gate::resolve(&gw, &headers, &sink).await?;
    let meta = body.map(|Json(m)| m).unwrap_or_default();
    let verdict = s.complete(&id, &meta).await.map_err(|e| gate::status(&e))?;
    Ok(Json(verdict))
}

/// `DELETE /uploads/{sink}/{id}` — discard a partial upload.
pub async fn abort_upload(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((sink, id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let (s, _p) = gate::resolve(&gw, &headers, &sink).await?;
    s.abort(&id).await.map_err(|e| gate::status(&e))?;
    Ok(Json(serde_json::json!({ "aborted": true })))
}
