//! Admin grants/roles routes — read + assign/revoke over the gateway (admin-console scope: roles &
//! grants are read + assign/revoke only this pass, no role editor). Mirror `lb_host::grants_*` /
//! `roles_*`; gated server-side on `mcp:grants.assign:call` / `mcp:grants.list:call` /
//! `mcp:roles.list:call`. The subject arrives as a `kind:name` string and is parsed here.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{AuthzRole, Subject};
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// The `?subject=user:ada` query for `GET /admin/grants`.
#[derive(Debug, Deserialize)]
pub struct SubjectQuery {
    pub subject: String,
}

/// `GET /admin/grants?subject=…` — the caps a subject holds directly.
pub async fn list_grants(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Query(q): Query<SubjectQuery>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let subject = parse_subject(&q.subject)?;
    let caps = lb_host::grants_list(&gw.node.store, &p, p.ws(), &subject)
        .await
        .map_err(forbid)?;
    Ok(Json(caps))
}

/// `GET /admin/roles` — every role defined in the workspace.
pub async fn list_roles(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<AuthzRole>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let roles = lb_host::roles_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(forbid)?;
    Ok(Json(roles))
}

/// The `POST /admin/grants` (assign) / `DELETE`-style revoke body: subject + cap.
#[derive(Debug, Deserialize)]
pub struct GrantBody {
    pub subject: String,
    pub cap: String,
}

/// `POST /admin/grants` — assign a cap (or `role:<name>`) to a subject.
pub async fn assign_grant(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<GrantBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let subject = parse_subject(&body.subject)?;
    lb_host::grants_assign(&gw.node.store, &p, p.ws(), &subject, &body.cap)
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /admin/grants/revoke` — revoke a cap from a subject.
pub async fn revoke_grant(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<GrantBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let subject = parse_subject(&body.subject)?;
    lb_host::grants_revoke(&gw.node.store, &p, p.ws(), &subject, &body.cap)
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

fn parse_subject(raw: &str) -> Result<Subject, (StatusCode, String)> {
    Subject::parse(raw).ok_or_else(|| (StatusCode::BAD_REQUEST, format!("bad subject: {raw}")))
}

fn forbid(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, e.to_string())
}
