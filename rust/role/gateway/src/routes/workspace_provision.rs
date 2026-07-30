//! `POST /workspaces/{ws}/provision` + `POST /workspaces/{ws}/reconcile` — the atomic workspace
//! provision + orphan-repair pair (workspace-provision scope, NubeDev/lb#121). Mirror
//! `lb_host::workspace_provision` / `workspace_reconcile` 1:1. The target `ws` is the path object;
//! authorization runs against the CALLER's own workspace, so the caller's session is untouched and
//! the reply carries no token. Command-shaped: a POST returning the outcome report.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{ProvisionReport, ReconcileReport, WorkspacesError};
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// The `POST /workspaces/{ws}/provision` body: display name + optional first admin. NO credential
/// field — onboarding a person who doesn't exist yet is the invites scope's seam.
#[derive(Debug, Deserialize)]
pub struct ProvisionWorkspace {
    pub name: String,
    #[serde(default)]
    pub admin: Option<String>,
}

/// `POST /workspaces/{ws}/provision` — stand up a complete workspace in one call.
pub async fn provision_workspace(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ws): Path<String>,
    Json(body): Json<ProvisionWorkspace>,
) -> Result<Json<ProvisionReport>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let report = lb_host::workspace_provision(
        &gw.node.store,
        &p,
        &ws,
        &body.name,
        body.admin.as_deref(),
        None,
        gw.now(),
    )
    .await
    .map_err(status)?;
    Ok(Json(report))
}

/// The `POST /workspaces/{ws}/reconcile` body: optional admin to install (defaults to the caller).
#[derive(Debug, Default, Deserialize)]
pub struct ReconcileWorkspace {
    #[serde(default)]
    pub admin: Option<String>,
}

/// `POST /workspaces/{ws}/reconcile` — repair a listable-but-memberless orphan.
pub async fn reconcile_workspace(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ws): Path<String>,
    body: Option<Json<ReconcileWorkspace>>,
) -> Result<Json<ReconcileReport>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let admin = body.and_then(|Json(b)| b.admin);
    let report = lb_host::workspace_reconcile(&gw.node.store, &p, &ws, admin.as_deref(), gw.now())
        .await
        .map_err(status)?;
    Ok(Json(report))
}

/// Typed error → status: denial is opaque 403; invalid input 422; a purged tombstone 409; a torn
/// provision write 500 (the workspace is absent from the directory — retrying the same id is safe).
fn status(e: WorkspacesError) -> (StatusCode, String) {
    let code = match &e {
        WorkspacesError::Denied => StatusCode::FORBIDDEN,
        WorkspacesError::Invalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
        WorkspacesError::Purged => StatusCode::CONFLICT,
        WorkspacesError::Store(_) | WorkspacesError::ProvisionFailed { .. } => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    (code, e.to_string())
}
