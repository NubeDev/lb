//! Admin grants/roles routes — read + assign/revoke over the gateway (admin-console scope: roles &
//! grants are read + assign/revoke only this pass, no role editor). Mirror `lb_host::grants_*` /
//! `roles_*`; gated server-side on `mcp:grants.assign:call` / `mcp:grants.list:call` /
//! `mcp:roles.list:call`. The subject arrives as a `kind:name` string and is parsed here.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{authz_resolve, revoke_tokens, roles_delete, AuthzRole, Scope, SourcedCap, Subject};
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
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
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
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let roles = lb_host::roles_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(forbid)?;
    Ok(Json(roles))
}

/// The `POST /admin/roles` (define) body: a role name and the caps it bundles.
#[derive(Debug, Deserialize)]
pub struct RoleBody {
    pub name: String,
    pub caps: Vec<String>,
}

/// `POST /admin/roles` — define (or replace) a custom role bundling `caps`. No-widening is enforced
/// server-side (`roles_define`): the definer may only bundle caps they themselves hold.
pub async fn define_role(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<RoleBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::roles_define(&gw.node.store, &p, p.ws(), &body.name, &body.caps)
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
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
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let subject = parse_subject(&body.subject)?;
    lb_host::grants_assign(&gw.node.store, &p, p.ws(), &subject, &body.cap, &Scope::All)
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
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let subject = parse_subject(&body.subject)?;
    lb_host::grants_revoke(&gw.node.store, &p, p.ws(), &subject, &body.cap, &Scope::All)
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

// ── access-console scope — the three verbs that close the access-graph gaps: resolved effective
//    caps WITH provenance (read), the live-token revoke lever, and roles.delete cascade. Each
//    re-checks its admin cap server-side via the `lb_host` verb; ws + principal from the token. ──

/// `GET /admin/authz/resolve?subject=…` — the subject's resolved effective caps, each tagged with
/// its source (direct / role:r / via team:t). Gated `mcp:authz.resolve:call` server-side.
pub async fn resolve_caps(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Query(q): Query<SubjectQuery>,
) -> Result<Json<Vec<SourcedCap>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let subject = parse_subject(&q.subject)?;
    let caps = authz_resolve(&gw.node.store, &p, p.ws(), &subject)
        .await
        .map_err(forbid)?;
    Ok(Json(caps))
}

/// `POST /admin/authz/revoke-tokens` — kill the subject's live tokens (verify-path marker) AND
/// tombstone its grants, for a full immediate lockout. Gated `mcp:authz.revoke-tokens:call`.
pub async fn revoke_tokens_route(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SubjectBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let subject = parse_subject(&body.subject)?;
    let revoked = revoke_tokens(&gw.node.store, &p, p.ws(), &subject)
        .await
        .map_err(forbid)?;
    Ok(Json(serde_json::json!({ "grants_revoked": revoked })))
}

/// The `POST /admin/authz/revoke-tokens` body: just the target subject.
#[derive(Debug, Deserialize)]
pub struct SubjectBody {
    pub subject: String,
}

/// `DELETE /admin/roles/{name}` — delete a custom role, cascade-un-assigning it from every subject.
/// Built-ins are immutable (rejected with a clear `400`). Gated `mcp:roles.manage:call`.
pub async fn delete_role(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    name: axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    match roles_delete(&gw.node.store, &p, p.ws(), &name).await {
        Ok(affected) => Ok(Json(serde_json::json!({ "affected": affected }))),
        Err(lb_host::AuthzError::Immutable(r)) => Err((
            StatusCode::BAD_REQUEST,
            format!("built-in role is immutable: {r}"),
        )),
        Err(e) => Err(forbid(e)),
    }
}
