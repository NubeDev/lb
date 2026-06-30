//! `GET /channels` + `POST /channels` — the channel switcher's list + create (collaboration scope,
//! slice 2). Mirrors `lb_host::channel_list` / `channel_create`. Distinct from the message routes
//! (`/channels/{cid}/messages`): these manage the *registry*, not the messages. Authenticated by the
//! session token; gated by the channel `sub`/`pub` caps the host verbs check.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::ChannelRecord;
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /channels` — every registered channel in the session's workspace.
pub async fn list_channels(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<ChannelRecord>>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let records = lb_host::channel_list(&gw.node.store, &principal, principal.ws())
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(records))
}

/// The `POST /channels` body: the channel id to register.
#[derive(Debug, Deserialize)]
pub struct CreateChannel {
    pub channel: String,
}

/// `POST /channels` — explicitly register a channel so it is listable before any post.
pub async fn create_channel(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<CreateChannel>,
) -> Result<Json<ChannelRecord>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let record = lb_host::channel_create(
        &gw.node.store,
        &principal,
        principal.ws(),
        &body.channel,
        gw.now(),
    )
    .await
    .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(record))
}
