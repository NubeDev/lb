//! `GET /uploads/{sink}/{id}` — how far the sink got.
//!
//! **Offsets are the sink's truth**, and lb holds no upload state of its own — which is exactly why
//! resumption survives an lb restart for free (scope decision 5): there is nothing here to lose.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;

use super::gate;
use crate::state::Gateway;

/// `GET /uploads/{sink}/{id}` → `{id, offset}`.
pub async fn upload_status(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((sink, id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let (s, _p) = gate::resolve(&gw, &headers, &sink).await?;
    let handle = s.status(&id).await.map_err(|e| gate::status(&e))?;
    Ok(Json(
        serde_json::json!({ "id": handle.id, "offset": handle.offset }),
    ))
}
