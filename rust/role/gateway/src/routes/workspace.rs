//! `GET /workspaces` + `POST /workspaces` — the workspace switcher's list + create (collaboration
//! scope, slice 2). Mirrors `lb_host::workspace_list` / `workspace_create` one-to-one. Authenticated
//! by the session token; the verbs gate on `mcp:workspace.<verb>:call` in the session's workspace.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::WorkspaceRecord;
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /workspaces` — every workspace in the node directory.
pub async fn list_workspaces(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<WorkspaceRecord>>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let records = lb_host::workspace_list(&gw.node.store, &principal)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(records))
}

/// The `POST /workspaces` body: the workspace id + display name to register.
#[derive(Debug, Deserialize)]
pub struct CreateWorkspace {
    pub ws: String,
    pub name: String,
}

/// `POST /workspaces` — register a workspace in the directory.
pub async fn create_workspace(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<CreateWorkspace>,
) -> Result<Json<WorkspaceRecord>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let record = lb_host::workspace_create(&gw.node.store, &principal, &body.ws, &body.name, gw.now)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(record))
}
