//! `GET /channels/{cid}/stream?token=<jwt>` — the **server→browser** push the whole live-UI story
//! rides on (README §6.13). The browser opens this once and receives:
//!   - `event: message` — each live channel [`Item`] as it is posted OR edited (others' messages
//!     appear; edits update in place via the UI's id-keyed merge);
//!   - `event: delete` — `{ id }` as one of the channel's messages is deleted by its author;
//!   - `event: presence` — `{member, present}` as members join/leave (Zenoh liveliness).
//!
//! Authentication is by a `?token=` **query param**, not a bearer header: the browser opens SSE with
//! `EventSource`, which cannot set headers. The token is verified identically (`session::verify_token`)
//! — workspace + caps come from it (§7) — so an unauthenticated session gets `401` before any stream
//! opens, and the `sub` grant is then checked by the host verbs (a `403` if ungranted).

use std::convert::Infallible;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use serde::Deserialize;
use serde_json::json;

use crate::session::verify_token;
use crate::state::Gateway;

/// The SSE auth query param: the session token (`EventSource` can't send a bearer header).
#[derive(Debug, Deserialize)]
pub struct StreamAuth {
    #[serde(default)]
    pub token: String,
}

/// Open the SSE stream for channel `cid`. `401` if the token is missing/bad; `403` if the session
/// lacks the `sub` grant (the host's check). The browser never gets a stream it isn't authorized for.
pub async fn channel_stream(
    State(gw): State<Gateway>,
    Path(cid): Path<String>,
    Query(auth): Query<StreamAuth>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let principal = verify_token(&gw, &auth.token)
        .await
        .map_err(|e| e.into_response())?;
    let ws = principal.ws().to_string();

    // Authorize + declare all three feeds up front (workspace-first). A denial here is a 403,
    // before any stream body is produced.
    let sub = lb_host::subscribe_channel(&gw.node.bus, &principal, &ws, &cid)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    let deletions = lb_host::watch_deletions(&gw.node.bus, &principal, &ws, &cid)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    let presence = lb_host::watch(&gw.node.bus, &principal, &ws, &cid)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;

    // Merge the three feeds into one SSE event stream.
    let stream = futures::stream::unfold(
        (sub, deletions, presence),
        |(sub, deletions, presence)| async move {
            let event = tokio::select! {
                item = sub.recv() => item.map(|i| {
                    Event::default()
                        .event("message")
                        .json_data(&i)
                        .unwrap_or_else(|_| Event::default().comment("encode error"))
                }),
                id = deletions.recv() => id.map(|id| {
                    Event::default()
                        .event("delete")
                        .data(json!({ "id": id }).to_string())
                }),
                change = presence.recv() => change.map(|(member, present)| {
                    Event::default()
                        .event("presence")
                        .data(json!({ "member": member, "present": present }).to_string())
                }),
            };
            event.map(|e| (Ok(e), (sub, deletions, presence)))
        },
    );

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
