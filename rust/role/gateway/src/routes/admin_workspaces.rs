//! Admin workspace lifecycle routes — rename / archive / purge over the gateway (admin-crud scope).
//! Mirror `lb_host::workspace_*` 1:1; gated server-side on `mcp:workspace.delete:call` (rename/soft)
//! and the distinct `mcp:workspace.purge:call` + a typed confirm token (hard). The UI cap-gate is
//! convenience; these routes are the boundary.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// The `POST /admin/workspaces/{ws}/rename` body.
#[derive(Debug, Deserialize)]
pub struct RenameWorkspace {
    pub name: String,
}

/// `POST /admin/workspaces/{ws}/rename` — set the display name + un-archive.
pub async fn rename_workspace(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ws): Path<String>,
    Json(body): Json<RenameWorkspace>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::workspace_rename(&gw.node.store, &p, &ws, &body.name, gw.now())
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /admin/workspaces/{ws}/archive` — soft-delete (reversible).
pub async fn archive_workspace(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ws): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::workspace_delete(&gw.node.store, &p, &ws)
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// The `POST /admin/workspaces/{ws}/purge` body: the typed confirm token (must equal `ws`).
#[derive(Debug, Deserialize)]
pub struct PurgeWorkspace {
    pub confirm: String,
}

/// `POST /admin/workspaces/{ws}/purge` — hard-delete (irreversible). Needs the purge cap + confirm.
pub async fn purge_workspace(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ws): Path<String>,
    Json(body): Json<PurgeWorkspace>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::workspace_purge(&gw.node.store, &p, &ws, &body.confirm)
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

fn forbid(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, e.to_string())
}
