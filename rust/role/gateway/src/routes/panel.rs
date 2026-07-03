//! Panel routes — the browser's `panel.*` surface over the gateway (library-panels scope, build step
//! 3). Each route mirrors a `lb_host::panel_*` verb 1:1 and re-runs the host's three gates server-side
//! (workspace-first → `mcp:panel.<verb>:call` → membership/visibility). The workspace + owner come from
//! the **token**, never the body (§7) — so a panel's owner is the authenticated principal,
//! un-spoofable. The UI cap-gate is convenience only; this is the boundary.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{PanelError, PanelSpec, PanelVisibility};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /panels` — the roster the caller can reach (own + team-shared + workspace), summaries only (no
/// spec bodies, no usage count). Gated `panel.list`.
pub async fn list_panels(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let rows = lb_host::panel_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(status)?;
    Ok(Json(json!({ "panels": rows })))
}

/// `GET /panels/{id}` — one panel (the three-gate read, full spec). Gated `panel.get`.
pub async fn get_panel(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let panel = lb_host::panel_get(&gw.node.store, &p, p.ws(), &id)
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(panel).unwrap_or(Value::Null)))
}

/// `POST /panels` body — create/update a panel (UPSERT on `id`). Owner is the token's principal;
/// visibility is set via `/share`, never here.
#[derive(Debug, Deserialize)]
pub struct SavePanel {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub spec: PanelSpec,
}

/// `POST /panels` — idempotent UPSERT (create on a fresh id, owner-only update). Gated `panel.save`.
pub async fn save_panel(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SavePanel>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let panel = lb_host::panel_save(
        &gw.node.store,
        &p,
        p.ws(),
        &body.id,
        &body.title,
        body.spec,
        gw.now(),
    )
    .await
    .map_err(status)?;
    Ok(Json(serde_json::to_value(panel).unwrap_or(Value::Null)))
}

/// `DELETE /panels/{id}?force=` — idempotent tombstone (owner-only; refused while in use unless
/// `force`). Gated `panel.delete`.
#[derive(Debug, Deserialize, Default)]
pub struct DeleteQuery {
    #[serde(default)]
    pub force: bool,
}

pub async fn delete_panel(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<DeleteQuery>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::panel_delete(&gw.node.store, &p, p.ws(), &id, q.force, gw.now())
        .await
        .map_err(status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /panels/{id}/share` body — set visibility (`private|team|workspace`) + optional team.
#[derive(Debug, Deserialize)]
pub struct SharePanel {
    pub visibility: String,
    #[serde(default)]
    pub team: Option<String>,
}

pub async fn share_panel(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SharePanel>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let visibility = parse_visibility(&body.visibility).ok_or((
        StatusCode::BAD_REQUEST,
        format!("bad visibility: {}", body.visibility),
    ))?;
    let panel = lb_host::panel_share(
        &gw.node.store,
        &p,
        p.ws(),
        &id,
        visibility,
        body.team.as_deref(),
        gw.now(),
    )
    .await
    .map_err(status)?;
    Ok(Json(serde_json::to_value(panel).unwrap_or(Value::Null)))
}

/// `GET /panels/{id}/usage` — the dashboards referencing this panel (delete-safety + editor banner).
/// Gated `panel.usage`.
pub async fn panel_usage(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let rows = lb_host::panel_usage(&gw.node.store, &p, p.ws(), &id)
        .await
        .map_err(status)?;
    Ok(Json(json!({ "usage": rows })))
}

fn parse_visibility(s: &str) -> Option<PanelVisibility> {
    match s {
        "private" => Some(PanelVisibility::Private),
        "team" => Some(PanelVisibility::Team),
        "workspace" => Some(PanelVisibility::Workspace),
        _ => None,
    }
}

/// Map a panel gate outcome onto an HTTP status. `Denied` is `403` (opaque); `NotFound` `404`;
/// `BadInput`/`InUse` `400`; a store fault is `403`-opaque like the other gateway routes.
fn status(e: PanelError) -> (StatusCode, String) {
    match e {
        PanelError::Denied => (StatusCode::FORBIDDEN, e.to_string()),
        PanelError::NotFound => (StatusCode::NOT_FOUND, e.to_string()),
        PanelError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        PanelError::InUse(rows) => (
            StatusCode::BAD_REQUEST,
            json!({ "error": "panel in use", "usage": rows }).to_string(),
        ),
        PanelError::Store(s) => (StatusCode::FORBIDDEN, s.to_string()),
    }
}
