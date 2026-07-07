//! Nav routes — the browser's `nav.*` surface over the gateway (nav scope, the "wired end to end"
//! deliverable). Each route mirrors an `lb_host::nav_*` verb 1:1 and re-runs the host's gates
//! server-side (workspace-first → `mcp:nav.<verb>:call` → membership/visibility). The workspace +
//! owner come from the **token**, never the body (§7) — so a nav's owner is the authenticated
//! principal, un-spoofable, and the per-user pick is keyed to the token's `sub` (a caller can never
//! curate another user's pick). The UI cap-gate is convenience only; this is the boundary.
//!
//! `nav.resolve` needs the whole `&Node` (it discovers `ext` items via `ext.list`), so those routes
//! pass `&gw.node`; the CRUD routes need only the store.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{NavError, NavItem, NavVisibility};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /navs` — the roster the caller can reach (own + team-shared + workspace), summaries only.
/// Gated `nav.list`.
pub async fn list_navs(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let rows = lb_host::nav_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(status)?;
    Ok(Json(json!({ "navs": rows })))
}

/// `GET /navs/{id}` — one nav (the three-gate read, full `items[]`). Gated `nav.get`.
pub async fn get_nav(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let n = lb_host::nav_get(&gw.node.store, &p, p.ws(), &id)
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(n).unwrap_or(Value::Null)))
}

/// `GET /nav/resolve` — the caller's effective menu (picked, tag-expanded, cap-stripped). Gated
/// `nav.resolve`; member-level. THE one payload NavRail renders.
pub async fn resolve_nav(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let resolved = lb_host::nav_resolve(&gw.node, &p, p.ws())
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(resolved).unwrap_or(Value::Null)))
}

/// `POST /navs` body — create/update a nav (UPSERT on `id`). Owner is the token's principal.
#[derive(Debug, Deserialize)]
pub struct SaveNav {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub items: Vec<NavItem>,
}

/// `POST /navs` — idempotent UPSERT (create on a fresh id, owner-only update). Gated `nav.save`.
pub async fn save_nav(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SaveNav>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let n = lb_host::nav_save(
        &gw.node.store,
        &p,
        p.ws(),
        &body.id,
        &body.title,
        body.items,
        gw.now(),
    )
    .await
    .map_err(status)?;
    Ok(Json(serde_json::to_value(n).unwrap_or(Value::Null)))
}

/// `DELETE /navs/{id}` — idempotent tombstone (owner-only). Gated `nav.delete`.
pub async fn delete_nav(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::nav_delete(&gw.node.store, &p, p.ws(), &id, gw.now())
        .await
        .map_err(status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /navs/{id}/share` body — set visibility (`private|team|workspace`) + optional team.
#[derive(Debug, Deserialize)]
pub struct ShareNav {
    pub visibility: String,
    #[serde(default)]
    pub team: Option<String>,
}

/// `POST /navs/{id}/share` — set a nav's visibility / write the S4 share edge. Gated `nav.share`;
/// owner-only. Returns the updated nav.
pub async fn share_nav(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ShareNav>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let visibility = parse_visibility(&body.visibility).ok_or((
        StatusCode::BAD_REQUEST,
        format!("bad visibility: {}", body.visibility),
    ))?;
    let n = lb_host::nav_share(
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
    Ok(Json(serde_json::to_value(n).unwrap_or(Value::Null)))
}

/// `POST /navs/{id}/unshare` body — revoke one team share edge.
#[derive(Debug, Deserialize)]
pub struct UnshareNav {
    pub team: String,
}

/// `POST /navs/{id}/unshare` — revoke one S4 share edge (the inverse write). Gated `nav.share`
/// (same cap as `share`); owner-only. Idempotent. Returns the unchanged nav.
pub async fn unshare_nav(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UnshareNav>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let n = lb_host::nav_unshare(&gw.node.store, &p, p.ws(), &id, &body.team, gw.now())
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(n).unwrap_or(Value::Null)))
}

/// `GET /navs/{id}/shares` — enumerate the live team shares. Gated `nav.share`; owner-only.
pub async fn list_shares_nav(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let teams = lb_host::nav_list_shares(&gw.node.store, &p, p.ws(), &id)
        .await
        .map_err(status)?;
    Ok(Json(json!({ "teams": teams })))
}

/// `POST /nav/default` body — set the workspace-default nav (empty `id` clears it).
#[derive(Debug, Deserialize)]
pub struct SetDefaultNav {
    pub id: String,
}

/// `POST /nav/default` — set the one workspace-default pointer. Gated `nav.save` (admin-ish).
pub async fn set_default_nav(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SetDefaultNav>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::nav_set_default(&gw.node.store, &p, p.ws(), &body.id, gw.now())
        .await
        .map_err(status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /nav/pref` — read the caller's own active-nav pick. Gated `nav.resolve`; member-level.
pub async fn get_nav_pref(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let pref = lb_host::nav_pref_get(&gw.node.store, &p, p.ws())
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(pref).unwrap_or(Value::Null)))
}

/// `POST /nav/pref` body — set the caller's own active-nav pick (empty `id` clears it).
#[derive(Debug, Deserialize)]
pub struct SetNavPref {
    pub id: String,
}

/// `POST /nav/pref` — set the caller's own active-nav pick. Keyed to the token `sub` (a caller cannot
/// set another user's pick). Gated `nav.resolve`; member-level.
pub async fn set_nav_pref(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SetNavPref>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let pref = lb_host::nav_pref_set(&gw.node.store, &p, p.ws(), &body.id, gw.now())
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(pref).unwrap_or(Value::Null)))
}

fn parse_visibility(s: &str) -> Option<NavVisibility> {
    match s {
        "private" => Some(NavVisibility::Private),
        "team" => Some(NavVisibility::Team),
        "workspace" => Some(NavVisibility::Workspace),
        _ => None,
    }
}

/// Map a nav gate outcome onto an HTTP status. `Denied` is `403` (opaque); `NotFound` `404`;
/// `BadInput` `400`; a store fault is `403`-opaque like the other gateway routes.
fn status(e: NavError) -> (StatusCode, String) {
    match e {
        NavError::Denied => (StatusCode::FORBIDDEN, e.to_string()),
        NavError::NotFound => (StatusCode::NOT_FOUND, e.to_string()),
        NavError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        NavError::Store(s) => (StatusCode::FORBIDDEN, s.to_string()),
    }
}
