//! `GET /runs/{job}/stream?token=<jwt>` — the **agent-run live feed** the browser UI watches
//! (agent-run scope Part 3). Mirrors `channel_stream` (`routes/stream.rs`): the browser opens this
//! once with `EventSource` and receives the run's `RunEvent`s — `text-delta`, `tool-call-start`,
//! `skill-activated`, `suspended`, `run-finish` — instead of only a final answer.
//!
//! **A late join gets a snapshot then deltas** (review point 5): `watch_run` returns the projection
//! of the durable transcript so far (the catch-up) plus the live subscription. This route emits the
//! snapshot events first, then folds the live stream — so a UI that opens mid-run reconstructs state
//! from the record, never from deltas it missed.
//!
//! Auth is the `?token=` query param (`EventSource` can't set headers), verified identically to the
//! channel stream; the workspace + caps come from the token (§7). `watch_run` then checks
//! `mcp:agent.watch:call` (a `403` before any stream body) and the bus subject is workspace-walled,
//! so a ws-B session can never observe a ws-A run.

use std::convert::Infallible;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use lb_run_events::RunEvent;

use crate::routes::stream::StreamAuth;
use crate::session::verify_token;
use crate::state::Gateway;

/// Open the SSE stream for run `job`. `401` if the token is missing/bad; `403` if the session lacks
/// `mcp:agent.watch:call` or the run is cross-workspace. Emits `event: run` frames, each carrying one
/// JSON-encoded [`RunEvent`].
pub async fn run_stream(
    State(gw): State<Gateway>,
    Path(job): Path<String>,
    Query(auth): Query<StreamAuth>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let principal = verify_token(&gw, &auth.token)
        .await
        .map_err(|e| e.into_response())?;
    let ws = principal.ws().to_string();

    // Authorize + read the snapshot + declare the live feed up front (workspace-first). A denial here
    // is a 403, before any stream body is produced.
    let watch = lb_host::watch_run(&gw.node.store, &gw.node.bus, &principal, &ws, &job)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;

    // The stream state: first drain the catch-up snapshot (already in memory), then the live feed.
    let snapshot = watch.snapshot.into_iter();
    let stream = futures::stream::unfold(
        (snapshot, watch.stream, false),
        |(mut snapshot, live, snapshot_done)| async move {
            // Phase 1: emit the snapshot events the watcher missed before attaching.
            if let Some(event) = snapshot.next() {
                return Some((Ok(sse_event(&event)), (snapshot, live, snapshot_done)));
            }
            // Phase 2: fold the live delta feed until it closes.
            live.recv()
                .await
                .map(|event| (Ok(sse_event(&event)), (snapshot, live, true)))
        },
    );

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Encode one [`RunEvent`] as an `event: run` SSE frame carrying its JSON. A serialization failure
/// degrades to a comment frame (never breaks the stream).
fn sse_event(event: &RunEvent) -> Event {
    Event::default()
        .event("run")
        .json_data(event)
        .unwrap_or_else(|_| Event::default().comment("encode error"))
}
