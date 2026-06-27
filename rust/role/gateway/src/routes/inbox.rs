//! `GET /inbox/{channel}` + `POST /inbox/{item}/resolve` — the real inbox view's list + resolve
//! (collaboration scope, slice 4). Mirrors `lb_host::list_inbox` / `resolve_inbox`. This is the
//! durable `lb-inbox` surface that replaces the workflow fake's simulated inbox. Authenticated by the
//! session token; gated by `mcp:inbox.<verb>:call`.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_inbox::{Decision, Item};
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /inbox/{channel}` — the durable items of an inbox channel.
pub async fn list_inbox(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(channel): Path<String>,
) -> Result<Json<Vec<Item>>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let items = lb_host::list_inbox(&gw.node.store, &principal, principal.ws(), &channel)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(items))
}

/// The `POST /inbox/{item}/resolve` body: the reviewer's decision.
#[derive(Debug, Deserialize)]
pub struct ResolveInbox {
    pub decision: Decision,
}

/// `POST /inbox/{item}/resolve` — record an approve/reject/defer on an inbox item. The S6 approval
/// gate as a real UI action.
pub async fn resolve_inbox(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(item): Path<String>,
    Json(body): Json<ResolveInbox>,
) -> Result<StatusCode, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    lb_host::resolve_inbox(
        &gw.node.store,
        &principal,
        principal.ws(),
        &item,
        body.decision,
        gw.now,
    )
    .await
    .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
