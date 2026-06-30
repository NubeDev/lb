//! Admin user routes — the dev-store user CRUD over the gateway (admin-crud scope). Mirror
//! `lb_host::user_*` 1:1; authenticated by the session token; gated server-side on
//! `mcp:user.manage:call` / `mcp:user.disable:call` in the session's workspace. The UI cap-gate is
//! convenience only — these routes are the truth (a forged call by a non-admin is denied here).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::UserView;
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /admin/users` — every user in the session's workspace (credential-free views).
pub async fn list_users(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserView>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let users = lb_host::user_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(forbid)?;
    Ok(Json(users))
}

/// The `POST /admin/users` body: the user to create + optional role.
#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub user: String,
    #[serde(default)]
    pub role: Option<String>,
}

/// `POST /admin/users` — seed a dev user record.
pub async fn create_user(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<CreateUser>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let role = body.role.as_deref().unwrap_or("member");
    lb_host::user_create(
        &gw.node.store,
        &p,
        p.ws(),
        &body.user,
        role,
        "dev",
        gw.now(),
    )
    .await
    .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /admin/users/{user}/disable` — flip `active=false` (the login path then refuses to mint).
pub async fn disable_user(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(user): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::user_disable(&gw.node.store, &p, p.ws(), &user)
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /admin/users/{user}/enable` — restore minting.
pub async fn enable_user(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(user): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::user_enable(&gw.node.store, &p, p.ws(), &user)
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /admin/users/{user}` — workspace-scoped delete + grant revoke. Returns the revoked count.
pub async fn delete_user(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(user): Path<String>,
) -> Result<Json<usize>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let revoked = lb_host::user_delete(&gw.node.store, &p, p.ws(), &user)
        .await
        .map_err(forbid)?;
    Ok(Json(revoked))
}

fn forbid(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, e.to_string())
}
