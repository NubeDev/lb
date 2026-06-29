//! Bus routes — the browser's generic `bus.publish` / `bus.watch` surface over the gateway (widget-
//! config-vars scope, "Platform fix"). Mirrors `ingest`/`series_stream`: `POST /bus/publish` is the
//! fire-and-forget motion sink (the JSON-payload builder's "over the bus" target), and
//! `GET /bus/{subject}/stream?token=` is the live subscribe (a cell/variable folding motion in).
//!
//! Each re-runs the host gate server-side (workspace-first, then `mcp:bus.publish|watch:call`); the
//! workspace + the subject wall come from the token (§7). A `bus.*` subject naming another workspace or
//! a reserved prefix (`series/`, `channels/`, internal) is refused by the host's `wall_subject` guard.
//! Both are ALSO reachable via `POST /mcp/call` (`bus.publish`) — this is the direct HTTP mirror.

use std::convert::Infallible;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::stream::Stream;
use lb_host::BusError;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::{authenticate, verify_token};
use crate::state::Gateway;

/// The bus-stream auth + target: the session token (`EventSource` can't set a bearer header) + the
/// subject (a query param, since a subject contains `/` and can't be a single path segment).
#[derive(Debug, Deserialize)]
pub struct BusStreamQuery {
    #[serde(default)]
    pub token: String,
    pub subject: String,
}

/// `POST /bus/publish` body — the subject + an opaque JSON payload. The subject is walled host-side
/// from the token; the payload is published as-is. Fire-and-forget (NOT durable, rule 3).
#[derive(Debug, Deserialize)]
pub struct PublishBody {
    pub subject: String,
    #[serde(default)]
    pub payload: Value,
}

fn bus_status(e: BusError) -> (StatusCode, String) {
    match e {
        BusError::Denied => (StatusCode::FORBIDDEN, "denied".into()),
        BusError::BadSubject(m) | BusError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        BusError::Bus(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
    }
}

/// `POST /bus/publish` — publish `payload` onto `subject` as the token's principal. Gated
/// `mcp:bus.publish:call`, workspace-walled. Returns `{ ok: true }` — "handed to the bus", never an ack.
pub async fn publish_message(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<PublishBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let bytes =
        serde_json::to_vec(&body.payload).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    lb_host::bus_publish(&gw.node.bus, &p, p.ws(), &body.subject, &bytes)
        .await
        .map_err(bus_status)?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /bus/stream?subject=<s>&token=<jwt>` — subscribe to live payloads on `subject`. `401` if the
/// token is missing/bad; `403`/`400` if the session lacks `mcp:bus.watch:call` or the subject is reserved
/// — both before any stream body. Each frame is `event: message` carrying the JSON payload verbatim. The
/// subject is a query param (it contains `/`, so it cannot be a single path segment).
pub async fn bus_stream(
    State(gw): State<Gateway>,
    Query(q): Query<BusStreamQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let principal = verify_token(&gw, &q.token)
        .await
        .map_err(|e| e.into_response())?;
    let ws = principal.ws().to_string();

    let sub = lb_host::bus_watch(&gw.node.bus, &principal, &ws, &q.subject)
        .await
        .map_err(bus_status)?;

    let stream = futures::stream::unfold(sub, |sub| async move {
        let event = sub.recv().await.map(|bytes| {
            // The payload is the JSON the publisher sent; emit it as parsed data (or the raw text).
            let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            Event::default()
                .event("message")
                .json_data(&value)
                .unwrap_or_else(|_| Event::default().comment("encode error"))
        });
        event.map(|e| (Ok(e), sub))
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
