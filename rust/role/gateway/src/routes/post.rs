//! `POST /channels/{cid}/messages` — the browser's "send a message" verb. Mirrors the Tauri
//! `channel_post` command and the UI's `channel.api.ts::post` one-to-one (same verb name across
//! transports). Thin glue over `lb_host::post` with the session principal — the capability check
//! is the host's, not the gateway's.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use lb_inbox::Item;

use crate::state::Gateway;

/// Post `item` to channel `cid`. Returns the stored item (channel filled in). A `Denied` (or any
/// host error) maps to 403 — the browser shows it exactly as the desktop shell would.
pub async fn post_message(
    State(gw): State<Gateway>,
    Path(cid): Path<String>,
    Json(item): Json<Item>,
) -> Result<Json<Item>, (StatusCode, String)> {
    let stored = lb_host::post(
        &gw.node.store,
        &gw.node.bus,
        &gw.principal,
        &gw.ws,
        &cid,
        item,
    )
    .await
    .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(stored))
}
