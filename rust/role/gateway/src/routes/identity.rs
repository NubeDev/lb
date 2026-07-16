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

/// The `POST /admin/identities` body. `email` is the optional login handle (email-login scope) —
/// claimed globally-unique when present.
#[derive(Debug, Deserialize)]
pub struct CreateIdentity {
    pub sub: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

/// `POST /admin/identities` — provision a global identity (in NO workspace), optionally with an email.
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
        body.email.as_deref(),
        gw.now(),
    )
    .await
    .map_err(identity_err)?;
    Ok(Json(view))
}

/// The `POST /admin/identities/{sub}/email` body.
#[derive(Debug, Deserialize)]
pub struct SetEmail {
    pub email: String,
}

/// `POST /admin/identities/{sub}/email` — set/change an identity's email login handle (email-login
/// scope). Gated `mcp:identity.manage:call`. A duplicate email is `409`.
pub async fn set_identity_email(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(sub): Path<String>,
    Json(body): Json<SetEmail>,
) -> Result<Json<IdentityView>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let view = lb_host::identity_set_email(&gw.node.store, &p, &sub, &body.email)
        .await
        .map_err(identity_err)?;
    Ok(Json(view))
}

/// The `POST /admin/identities/{sub}/password` body. The secret is never logged/echoed.
#[derive(Debug, Deserialize)]
pub struct SetPassword {
    pub secret: String,
}

/// `POST /admin/identities/{sub}/password` — admin-set a person's GLOBAL password (email-login
/// scope). Gated `mcp:identity.manage:call`. Returns `{ ok: true }`; never the hash.
pub async fn set_identity_password(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(sub): Path<String>,
    Json(body): Json<SetPassword>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::identity_set_password(&gw.node.store, &p, &sub, &body.secret, gw.now())
        .await
        .map_err(|e| match e {
            lb_host::IdentityCredentialError::Denied => (StatusCode::FORBIDDEN, "denied".into()),
            lb_host::IdentityCredentialError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
            other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
        })?;
    Ok(Json(serde_json::json!({ "ok": true })))
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

/// Map an identity-service error to HTTP: an email clash is `409 Conflict`, a denial `403`, a store
/// failure `500`. Keeps the email-uniqueness signal distinct from an authorization denial.
fn identity_err(e: lb_host::IdentityError) -> (StatusCode, String) {
    match e {
        lb_host::IdentityError::Denied => (StatusCode::FORBIDDEN, "denied".into()),
        lb_host::IdentityError::EmailTaken => (StatusCode::CONFLICT, "email already in use".into()),
        lb_host::IdentityError::Store(s) => (StatusCode::INTERNAL_SERVER_ERROR, s.to_string()),
    }
}
