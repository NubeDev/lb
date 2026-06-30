//! Identity admin routes — the global identity directory over the gateway (global-identity scope).
//! Mirror `lb_host::identity_*` 1:1; authenticated by the session token; gated server-side on
//! `mcp:identity.manage:call` in the session's workspace. The UI cap-gate is convenience only — these
//! routes are the truth (a forged call by a non-admin is denied here).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{IdentityView, IdentityWorkspace};
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /admin/identities` — every global identity.
pub async fn list_identities(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<IdentityView>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let identities = lb_host::identity_list(&gw.node.store, &p)
        .await
        .map_err(forbid)?;
    Ok(Json(identities))
}

/// The `POST /admin/identities` body.
#[derive(Debug, Deserialize)]
pub struct CreateIdentity {
    pub sub: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// `POST /admin/identities` — provision a global identity (in NO workspace).
pub async fn create_identity(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<CreateIdentity>,
) -> Result<Json<IdentityView>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let view = lb_host::identity_create(
        &gw.node.store,
        &p,
        &body.sub,
        body.display_name.as_deref(),
        gw.now,
    )
    .await
    .map_err(forbid)?;
    Ok(Json(view))
}

/// `GET /admin/identities/{sub}` — read one identity.
pub async fn get_identity(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(sub): Path<String>,
) -> Result<Json<Option<IdentityView>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let view = lb_host::identity_get(&gw.node.store, &p, &sub)
        .await
        .map_err(forbid)?;
    Ok(Json(view))
}

/// `GET /admin/identities/{sub}/workspaces` — the workspaces this identity belongs to.
pub async fn identity_workspaces(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(sub): Path<String>,
) -> Result<Json<Vec<IdentityWorkspace>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let workspaces = lb_host::identity_workspaces(&gw.node.store, &p, &sub)
        .await
        .map_err(forbid)?;
    Ok(Json(workspaces))
}

fn forbid(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, e.to_string())
}
