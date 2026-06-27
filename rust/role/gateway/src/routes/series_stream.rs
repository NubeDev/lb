//! `GET /series/{series}/stream?token=<jwt>` — the **live series feed** a dashboard widget watches
//! (dashboard scope, build step 4). The series analog of `stream.rs`: the browser opens it once per
//! distinct series and receives `event: sample` for each live `Sample` published onto the workspace's
//! `ws/{id}/series/{series}` motion subject (state vs motion, rule 3 — no polling `series.latest`).
//!
//! Auth is by a `?token=` query param (EventSource can't set a bearer header), verified identically
//! (`verify_token`) — workspace + caps come from it (§7). An unauthenticated session is `401` before
//! any stream opens; an ungranted one is `403` (the host's `series.read` check, workspace-first).

use std::convert::Infallible;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;

use super::stream::StreamAuth;
use crate::session::verify_token;
use crate::state::Gateway;

/// Open the SSE stream for `series`. `401` if the token is missing/bad; `403` if the session lacks
/// `mcp:series.read:call` (the host's check, before any bus interest is declared).
pub async fn series_stream(
    State(gw): State<Gateway>,
    Path(series): Path<String>,
    Query(auth): Query<StreamAuth>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let principal = verify_token(&gw, &auth.token).map_err(|e| e.into_response())?;
    let ws = principal.ws().to_string();

    // Authorize + declare the feed up front (workspace-first). A denial here is a 403, before any
    // stream body is produced.
    let sub = lb_host::subscribe_series(&gw.node.bus, &principal, &ws, &series)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;

    let stream = futures::stream::unfold(sub, |sub| async move {
        let event = sub.recv().await.map(|sample| {
            Event::default()
                .event("sample")
                .json_data(&sample)
                .unwrap_or_else(|_| Event::default().comment("encode error"))
        });
        event.map(|e| (Ok(e), sub))
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
