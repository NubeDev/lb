//! `POST /channels/{cid}/messages` — the browser's "send a message" verb. Mirrors the Tauri
//! `channel_post` command and the UI's `channel.api.ts::post` one-to-one (same verb name across
//! transports). Thin glue over `lb_host::post` with the **verified session principal** — the
//! capability check is the host's, and the workspace comes from the token (`session::authenticate`),
//! not the request (the hard wall, §7).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_inbox::Item;

use crate::session::authenticate;
use crate::state::Gateway;

/// Post `item` to channel `cid`. Returns the stored item (channel filled in). A `401` if the token
/// is missing/bad; a `403` if the host's capability check denies — the browser shows it as the
/// desktop shell would.
pub async fn post_message(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(cid): Path<String>,
    Json(item): Json<Item>,
) -> Result<Json<Item>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let stored = lb_host::post(
        &gw.node.store,
        &gw.node.bus,
        &principal,
        principal.ws(),
        &cid,
        item,
    )
    .await
    .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(stored))
}
