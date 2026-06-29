//! `GET /outbox` — the read-only delivery status view (collaboration scope, slice 4). Mirrors
//! `lb_host::outbox_status`. Returns the workspace's effects grouped pending / delivered /
//! dead-lettered so the UI shows "pending → delivered (→ dead-letter)". No mutation route exists —
//! the outbox is must-deliver infrastructure, never a CRUD surface. Gated by `mcp:outbox.status:call`.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::OutboxStatus;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /outbox` — the delivery snapshot for the session's workspace.
pub async fn get_outbox_status(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<OutboxStatus>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let status = lb_host::outbox_status(&gw.node.store, &principal, principal.ws())
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(status))
}
