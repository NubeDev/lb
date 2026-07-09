//! `GET /events/stream?token=` + `POST /events/{sid}/subscribe` + `POST /events/{sid}/unsubscribe` —
//! the browser's **one multiplexed SSE connection** (unified-event-stream scope §3). The whole live-UI
//! story rides this single connection: instead of one `EventSource` per run/channel/series (each
//! spending one of the browser's ~6 HTTP/1.1 slots — the pool-exhaustion defect), every feed is a
//! *subject* multiplexed here, freeing the rest for REST.
//!
//! - `GET /events/stream` — auth by `?token=` (`EventSource` can't set headers), mint a `sid`, emit
//!   `event: hello {sid}`, then fold the connection's queue into `event: mux` frames. The body carries a
//!   **drop guard** so closing the browser tab (or a reconnect) tears the connection down host-side
//!   (aborting its subject tasks — no leaked bus subscriptions).
//! - `POST /events/{sid}/subscribe {subject}` / `unsubscribe {subject}` — header-authed control verbs.
//!   Subscribe re-runs the subject's EXACT dedicated-route gate + workspace wall (via the subject
//!   registry); a deny is an opaque `event: mux {sub, event:"error"}` frame on the stream, never a
//!   connection kill. The workspace is the token's, so a subject naming another ws is the same opaque
//!   deny as an unknown one (the mux is not an existence oracle).

use std::convert::Infallible;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::stream::Stream;
use serde::Deserialize;
use serde_json::json;

use crate::routes::stream::StreamAuth;
use crate::session::{authenticate, verify_token};
use crate::state::Gateway;

/// `POST /events/{sid}/{un}subscribe` body — the opaque subject string (`run:{job}`, `channel:{cid}`,
/// `series:{s}`, `bus:{subject}`, `flow-run:{run}`, `flow-debug:{flow}`, `insights`, `telemetry`).
#[derive(Debug, Deserialize)]
pub struct SubjectBody {
    pub subject: String,
}

/// `GET /events/stream?token=<jwt>` — open the session's single multiplexed SSE connection. `401` if the
/// token is missing/bad (before any body). Emits `event: hello` carrying `{sid}` first, then one
/// `event: mux` frame per subject frame: `{"sub":<subject>,"event":<original name>,"data":<original
/// payload verbatim>}`. Subscriptions are added/removed by the control POSTs against `{sid}`.
pub async fn events_stream(
    State(gw): State<Gateway>,
    Query(auth): Query<StreamAuth>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    // Auth-first: the workspace + caps ride the token; a bad token is 401 before the connection registers.
    let _principal = verify_token(&gw, &auth.token)
        .await
        .map_err(|e| e.into_response())?;

    let (sid, rx) = gw.events.open().await;

    // The stream state: emit `hello` once, then drain the connection queue. A `CloseGuard` in the state
    // tears the connection down host-side when the browser drops the stream (the `unfold` state is
    // dropped), aborting every subject task so no bus subscription leaks.
    let guard = CloseGuard {
        hub: gw.events.clone(),
        sid: sid.clone(),
    };
    let init = json!({ "sid": sid }).to_string();
    let stream = futures::stream::unfold(
        (Some(init), rx, guard),
        |(hello, mut rx, guard)| async move {
            if let Some(hello) = hello {
                let ev = Event::default().event("hello").data(hello);
                return Some((Ok(ev), (None, rx, guard)));
            }
            match rx.recv().await {
                Some(line) => {
                    let ev = Event::default().event("mux").data(line);
                    Some((Ok(ev), (None, rx, guard)))
                }
                None => None,
            }
        },
    );

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// `POST /events/{sid}/subscribe {subject}` — add `subject` to connection `sid`. Header-authed; the
/// subject's gate re-runs server-side (the token's workspace). Always `200 {ok:true}` when the
/// connection exists — a gate DENY is reported as an opaque `error` frame ON the stream, not here (so a
/// caller can't probe subject existence via the control response). `404` only if `sid` is unknown (the
/// stream already dropped) — the client re-opens and re-subscribes.
pub async fn events_subscribe(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(sid): Path<String>,
    Json(body): Json<SubjectBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    gw.events
        .subscribe(&gw, &sid, &body.subject, &principal)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "no such stream".to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /events/{sid}/unsubscribe {subject}` — drop `subject` from `sid` (abort its feed task). Header-
/// authed; idempotent (`200` even if not subscribed or the connection is gone). This is the refcount-zero
/// path from the client hub.
pub async fn events_unsubscribe(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(sid): Path<String>,
    Json(body): Json<SubjectBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Authenticate (identity only — unsubscribe touches nothing gated; it just releases the caller's own
    // subscription). A bad token is 401.
    let _principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    gw.events.unsubscribe(&sid, &body.subject).await;
    Ok(Json(json!({ "ok": true })))
}

/// Drops with the SSE body: tears the connection down host-side (aborts all subject tasks, forgets the
/// sid) when the browser closes the stream or it drops on reconnect. Spawns the async `close` since
/// `Drop` is sync.
struct CloseGuard {
    hub: crate::session::events::EventHub,
    sid: String,
}

impl Drop for CloseGuard {
    fn drop(&mut self) {
        let hub = self.hub.clone();
        let sid = std::mem::take(&mut self.sid);
        tokio::spawn(async move { hub.close(&sid).await });
    }
}
