//! `PATCH` / `DELETE /channels/{cid}/messages/{id}` — the browser's edit and delete verbs for a
//! message its author owns. Mirrors the Tauri `channel_edit` / `channel_delete` commands and the
//! UI's `channel.api.ts::edit` / `::remove` one-to-one. Thin glue over `lb_host::edit` /
//! `lb_host::delete` with the **verified session principal** — the capability + ownership checks
//! are the host's, and the workspace comes from the token (`session::authenticate`), not the
//! request (the hard wall, §7).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_inbox::Item;
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// The PATCH body: just the new body text. Author, channel, and id come from the path and the
/// stored record (the host enforces ownership against the stored author, never this body).
#[derive(Debug, Deserialize)]
pub struct EditBody {
    pub body: String,
    /// The new logical ordering timestamp (caller-injected, not wall-clock — mirrors `Item::ts`).
    pub ts: u64,
}

/// Edit message `{id}` in channel `cid`. Returns the stored item. `401` if the token is
/// missing/bad; `403` on a capability or ownership denial; `404` if the caller owns an id that is
/// not present.
pub async fn edit_message(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((cid, id)): Path<(String, String)>,
    Json(body): Json<EditBody>,
) -> Result<Json<Item>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let stored = lb_host::edit(
        gw.node.as_ref(),
        &principal,
        principal.ws(),
        &cid,
        &id,
        &body.body,
        body.ts,
    )
    .await
    .map_err(|e| channel_status(&e))?;
    Ok(Json(stored))
}

/// Delete message `{id}` from channel `cid`. `204` on success; `401`/`403`/`404` as above.
pub async fn delete_message(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((cid, id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::delete(gw.node.as_ref(), &principal, principal.ws(), &cid, &id)
        .await
        .map_err(|e| channel_status(&e))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Map a channel error to its HTTP status. `Denied` is opaque (no detail) — a caller without
/// access learns nothing; `NotFound` is `404`; store/bus failures are `500`.
fn channel_status(e: &lb_host::ChannelError) -> (StatusCode, String) {
    match e {
        lb_host::ChannelError::Denied => (StatusCode::FORBIDDEN, e.to_string()),
        lb_host::ChannelError::NotFound => (StatusCode::NOT_FOUND, e.to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}
