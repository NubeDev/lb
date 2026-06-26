//! `GET /channels/{cid}/stream` — the **server→browser** push the whole S3 UI story rides on
//! (README §6.13: browsers receive updates over SSE). The browser opens this once and receives:
//!   - `event: message` — each live channel [`Item`] as it is posted (others' messages appear);
//!   - `event: presence` — `{member, present}` as members join/leave (Zenoh liveliness).
//!
//! Both come straight off the bus via the SAME capability-checked host verbs the rest of the
//! system uses (`subscribe_channel`, `watch`) — the gateway adds no new authority. Authorization
//! (a `bus:chan/{cid}:sub` grant, workspace-first) runs when the subscriptions are declared, so
//! an unauthorized browser session gets a 403 before any stream opens.

use std::convert::Infallible;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use serde_json::json;

use crate::state::Gateway;

/// Open the SSE stream for channel `cid`. Errors with 403 if the session lacks the `sub` grant
/// (the host's check) — the browser never gets a stream it isn't authorized for.
pub async fn channel_stream(
    State(gw): State<Gateway>,
    Path(cid): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    // Authorize + declare both feeds up front (workspace-first). A denial here is a 403, before
    // any stream body is produced.
    let sub = lb_host::subscribe_channel(&gw.node.bus, &gw.principal, &gw.ws, &cid)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    let presence = lb_host::watch(&gw.node.bus, &gw.principal, &gw.ws, &cid)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;

    // Merge the two feeds into one SSE event stream. `async_stream` would be tidier, but a hand
    // -rolled `unfold` keeps the dependency set minimal (one verb, no macro crate).
    let stream = futures::stream::unfold((sub, presence), |(sub, presence)| async move {
        // Race the next message against the next presence change; emit whichever arrives.
        let event = tokio::select! {
            item = sub.recv() => item.map(|i| {
                Event::default()
                    .event("message")
                    .json_data(&i)
                    .unwrap_or_else(|_| Event::default().comment("encode error"))
            }),
            change = presence.recv() => change.map(|(member, present)| {
                Event::default()
                    .event("presence")
                    .data(json!({ "member": member, "present": present }).to_string())
            }),
        };
        event.map(|e| (Ok(e), (sub, presence)))
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
