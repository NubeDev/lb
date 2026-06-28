//! Dashboard routes — the browser's `dashboard.*` surface over the gateway (dashboard scope, build
//! step 3). Each route mirrors a `lb_host::dashboard_*` verb 1:1 and re-runs the host's three gates
//! server-side (workspace-first → `mcp:dashboard.<verb>:call` → membership/visibility). The workspace
//! + owner come from the **token**, never the body (§7) — so a dashboard's owner is the authenticated
//! principal, un-spoofable. The UI cap-gate is convenience only; this is the boundary.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{Cell, DashboardError, DashboardVisibility};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /dashboards` — the roster the caller can reach (own + team-shared + workspace), summaries
/// only (no cell bodies). Gated `dashboard.list`.
pub async fn list_dashboards(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let rows = lb_host::dashboard_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(status)?;
    Ok(Json(json!({ "dashboards": rows })))
}

/// `GET /dashboards/{id}` — one dashboard (the three-gate read). Gated `dashboard.get`; a non-member
/// of a team-shared dashboard is `403`, an absent/tombstoned one `404`.
pub async fn get_dashboard(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let d = lb_host::dashboard_get(&gw.node.store, &p, p.ws(), &id)
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(d).unwrap_or(Value::Null)))
}

/// `POST /dashboards` body — create/update a dashboard (UPSERT on `id`). The owner is the token's
/// principal; visibility is set via `/share`, never here.
#[derive(Debug, Deserialize)]
pub struct SaveDashboard {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub cells: Vec<Cell>,
    /// Variable definitions (widget-config-vars Slice 2) — additive; a pre-variables client omits it.
    #[serde(default)]
    pub variables: Vec<lb_host::DashboardVariable>,
}

/// `POST /dashboards` — idempotent UPSERT (create on a fresh id, owner-only update). Gated
/// `dashboard.save`. Returns the persisted dashboard.
pub async fn save_dashboard(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SaveDashboard>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let d = lb_host::dashboard_save(
        &gw.node.store,
        &p,
        p.ws(),
        &body.id,
        &body.title,
        body.cells,
        body.variables,
        gw.now,
    )
    .await
    .map_err(status)?;
    Ok(Json(serde_json::to_value(d).unwrap_or(Value::Null)))
}

/// `DELETE /dashboards/{id}` — idempotent tombstone (owner-only). Gated `dashboard.delete`.
pub async fn delete_dashboard(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    lb_host::dashboard_delete(&gw.node.store, &p, p.ws(), &id, gw.now)
        .await
        .map_err(status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /dashboards/{id}/share` body — set visibility (`private|team|workspace`) + optional team.
#[derive(Debug, Deserialize)]
pub struct ShareDashboard {
    pub visibility: String,
    #[serde(default)]
    pub team: Option<String>,
}

/// `POST /dashboards/{id}/share` — set a dashboard's visibility / write the S4 share edge. Gated
/// `dashboard.share`; owner-only. Returns the updated dashboard.
pub async fn share_dashboard(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ShareDashboard>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let visibility = parse_visibility(&body.visibility).ok_or((
        StatusCode::BAD_REQUEST,
        format!("bad visibility: {}", body.visibility),
    ))?;
    let d = lb_host::dashboard_share(
        &gw.node.store,
        &p,
        p.ws(),
        &id,
        visibility,
        body.team.as_deref(),
        gw.now,
    )
    .await
    .map_err(status)?;
    Ok(Json(serde_json::to_value(d).unwrap_or(Value::Null)))
}

fn parse_visibility(s: &str) -> Option<DashboardVisibility> {
    match s {
        "private" => Some(DashboardVisibility::Private),
        "team" => Some(DashboardVisibility::Team),
        "workspace" => Some(DashboardVisibility::Workspace),
        _ => None,
    }
}

/// Map a dashboard gate outcome onto an HTTP status. `Denied` is `403` (opaque); `NotFound` `404`;
/// `BadInput` `400`; a store fault is `403`-opaque like the other gateway routes.
fn status(e: DashboardError) -> (StatusCode, String) {
    match e {
        DashboardError::Denied => (StatusCode::FORBIDDEN, e.to_string()),
        DashboardError::NotFound => (StatusCode::NOT_FOUND, e.to_string()),
        DashboardError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        DashboardError::Store(s) => (StatusCode::FORBIDDEN, s.to_string()),
    }
}
