//! `GET /telemetry/stream?token=<jwt>` — the **telemetry live tail** the console watches scroll
//! (telemetry-console scope). Its own route file (not `run_stream` reuse — one responsibility per
//! file), modeled on `run_stream.rs` + `series_stream.rs`: it shares the token-verify + ws-wall
//! helpers and folds a catch-up snapshot then the live bus feed.
//!
//! Auth is the `?token=` query param (`EventSource` can't set headers), verified identically to the
//! other streams; the workspace + caps come from the token (§7). `telemetry_tail` checks
//! `mcp:telemetry.read:call` (**403 before any body** if missing) and the bus subject is ws-walled,
//! so a ws-B session can never observe ws-A's telemetry — the read-surface wall the operator sink
//! legitimately doesn't have.

use std::convert::Infallible;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;

use super::stream::StreamAuth;
use crate::session::verify_token;
use crate::state::Gateway;

/// Open the telemetry tail SSE stream. `401` if the token is missing/bad; `403` if the session lacks
/// `mcp:telemetry.read:call` (the host's check, before any bus interest is declared). Emits the
/// catch-up snapshot as `event: snapshot` frames first, then each live row as `event: telemetry`.
pub async fn telemetry_stream(
    State(gw): State<Gateway>,
    Query(auth): Query<StreamAuth>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let principal = verify_token(&gw, &auth.token)
        .await
        .map_err(|e| e.into_response())?;
    let ws = principal.ws().to_string();

    // Authorize + read the snapshot + declare the live feed up front (workspace-first). A denial
    // here is a 403, before any stream body is produced.
    let (snapshot, sub) =
        lb_host::telemetry_tail(&gw.node.store, &gw.node.bus, &principal, &ws, 100)
            .await
            .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;

    // Phase 1: emit the snapshot rows the watcher missed before attaching; Phase 2: the live feed.
    let snap = snapshot.rows.into_iter();
    let stream = futures::stream::unfold(
        (snap, sub, false),
        |(mut snap, sub, snap_done)| async move {
            if let Some(row) = snap.next() {
                return Some((
                    Ok(Event::default()
                        .event("snapshot")
                        .json_data(&row)
                        .unwrap_or_else(|_| Event::default().comment("encode error"))),
                    (snap, sub, snap_done),
                ));
            }
            sub.recv().await.map(|bytes| {
                let data = String::from_utf8(bytes).unwrap_or_default();
                (
                    Ok(Event::default().event("telemetry").data(data)),
                    (snap, sub, true),
                )
            })
        },
    );

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
