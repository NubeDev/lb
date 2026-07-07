//! `GET /insights` + `GET /insights/{id}` + `POST /insights/{id}/{ack|resolve}` +
//! `GET /insights/{id}/occurrences` — the Insights page's REST surface (insights umbrella scope
//! + occurrences sub-scope). Mirrors `lb_host::insight_*`. Authenticated by the session token;
//! gated by `mcp:insight.<verb>:call`. The `ts`-taking verbs use `gw.now()` so the REST client
//! passes no `now` (the rules-messaging / dashboard-pin precedent).
//!
//! The live feed rides `GET /insights/events?token=<jwt>` (SSE over `ws/{ws}/insight/events`),
//! query-param authed like the channel stream (`EventSource` can't set headers).

use std::convert::Infallible;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::stream::Stream;
use lb_insights::{ListQuery, OccCursor};
use serde::Deserialize;

use crate::session::{authenticate, verify_token};
use crate::state::Gateway;

/// `GET /insights` — the faceted, keyset-paged list. Filter axes arrive as query params (the
/// `ListQuery` shape serialized flat); the page is newest-first.
pub async fn list_insights(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let page = lb_host::insight_list(&gw.node.store, &principal, principal.ws(), query)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(serde_json::to_value(page).unwrap_or_default()))
}

/// `GET /insights/{id}` — read one insight.
pub async fn get_insight(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let insight = lb_host::insight_get(&gw.node.store, &principal, principal.ws(), &id)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(serde_json::to_value(insight).unwrap_or_default()))
}

/// `POST /insights/{id}/ack` — ack the insight as the calling principal.
pub async fn ack_insight(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::insight_ack(&gw.node.store, &principal, principal.ws(), &id, gw.now())
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

/// The optional `note` body on `POST /insights/{id}/resolve`.
#[derive(Debug, Deserialize, Default)]
pub struct ResolveBody {
    #[serde(default)]
    pub note: Option<String>,
}

/// `POST /insights/{id}/resolve` — resolve the insight as the calling principal.
pub async fn resolve_insight(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Option<Json<ResolveBody>>,
) -> Result<StatusCode, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let note = body.and_then(|Json(b)| b.note);
    lb_host::insight_resolve(
        &gw.node.store,
        &principal,
        principal.ws(),
        &id,
        note.as_deref(),
        gw.now(),
    )
    .await
    .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /insights/{id}/occurrences` — the per-insight occurrence ring, newest-first. Pagination
/// via `?cursor.seq=…&limit=…`.
#[derive(Debug, Deserialize)]
pub struct OccParams {
    pub cursor: Option<OccCursor>,
    #[serde(default = "default_occ_limit")]
    pub limit: usize,
}

fn default_occ_limit() -> usize {
    50
}

/// `GET /insights/{id}/occurrences` — read the per-insight occurrence ring.
pub async fn list_occurrences(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<OccParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let page = lb_host::insight_occurrences(
        &gw.node.store,
        &principal,
        principal.ws(),
        &id,
        params.cursor,
        params.limit,
    )
    .await
    .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(serde_json::to_value(page).unwrap_or_default()))
}

/// The SSE auth query param (the session token — `EventSource` can't send a bearer header).
#[derive(Debug, Deserialize)]
pub struct EventsAuth {
    #[serde(default)]
    pub token: String,
}

/// `GET /insights/events?token=<jwt>` — the live raise/ack/resolve feed for the workspace, SSE over
/// the bus subject `ws/{ws}/insight/events`. `401` on a missing/bad token; `403` (opaque) without
/// `mcp:insight.watch:call` or across workspaces (the subject is ws-scoped — no cross-ws leak).
pub async fn insight_events(
    State(gw): State<Gateway>,
    Query(auth): Query<EventsAuth>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let principal = verify_token(&gw, &auth.token)
        .await
        .map_err(|e| e.into_response())?;
    let ws = principal.ws().to_string();
    let sub = lb_host::subscribe_insight_events(&gw.node.bus, &principal, &ws)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    let stream = futures::stream::unfold(sub, |sub| async move {
        let event = sub.recv().await.map(|ev| {
            Event::default()
                .event("message")
                .json_data(&ev)
                .unwrap_or_else(|_| Event::default().comment("encode error"))
        });
        event.map(|e| (Ok(e), sub))
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
