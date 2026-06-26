//! `GET /channels/{cid}/messages` — the browser's "read the durable history" verb. Mirrors the
//! Tauri `channel_history` command and `channel.api.ts::history`. Reads the durable record, so it
//! works across a node restart (state, §3.3). The same `sub`-grant check the host enforces.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use lb_inbox::Item;

use crate::state::Gateway;

/// Return channel `cid`'s items oldest→newest, for the session principal.
pub async fn get_history(
    State(gw): State<Gateway>,
    Path(cid): Path<String>,
) -> Result<Json<Vec<Item>>, (StatusCode, String)> {
    let items = lb_host::history(&gw.node.store, &gw.principal, &gw.ws, &cid)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(items))
}
