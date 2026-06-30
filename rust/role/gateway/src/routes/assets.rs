//! Shared-asset routes — the browser's `assets.*` surface over the gateway (S4 follow-up: the host
//! had the doc/skill verbs + the `assets.*` MCP bridge, but only the Tauri shell reached them, so a
//! browser threw `unknown command`). Each route mirrors `lb_host::<verb>` 1:1 and re-runs the host's
//! own gates server-side — workspace-first, then the `store:doc/*`/`store:skill/*` capability, then
//! the S4 membership/ownership/grant gate. The workspace + the principal come from the **token**,
//! never the request body (the hard wall, §7) — so the `ws`/`author` the in-memory fake passes are
//! simply ignored here.
//!
//! Status mapping: a gate refusal is `403` (opaque — a denied caller never learns the asset exists),
//! a `NotFound` (only reachable AFTER the gates pass) is `404`, a store fault is `403`-opaque like
//! the other gateway routes.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_assets::{Doc, Skill};
use lb_host::AssetError;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `POST /docs` body — create/overwrite a doc owned by the caller.
#[derive(Debug, Deserialize)]
pub struct PutDoc {
    pub id: String,
    pub title: String,
    pub content: String,
}

/// `POST /docs` — upsert a doc (owner = the token's principal). Returns `{ id }`.
pub async fn put_doc(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<PutDoc>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let d = lb_host::put_doc(
        &gw.node.store,
        &p,
        p.ws(),
        &body.id,
        &body.title,
        &body.content,
        lb_assets::ContentType::Text,
        &[],
        gw.now(),
    )
    .await
    .map_err(asset_status)?;
    Ok(Json(json!({ "id": d.id })))
}

/// `GET /docs` — the caller's own docs (id + title), the doc-list view.
pub async fn list_docs(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<Value>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let docs = lb_host::list_docs(&gw.node.store, &p, p.ws())
        .await
        .map_err(asset_status)?;
    Ok(Json(
        docs.iter()
            .map(|d| json!({ "id": d.id, "title": d.title }))
            .collect(),
    ))
}

/// `GET /docs/{id}` — read a doc the caller may see (owner / shared-team-member / linked-channel
/// sub-grantee — the S4 three gates). `403` if denied, `404` if absent (only after the gates pass).
pub async fn get_doc(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Doc>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let d = lb_host::get_doc(&gw.node.store, &p, p.ws(), &id)
        .await
        .map_err(asset_status)?;
    Ok(Json(d))
}

/// `POST /docs/{id}/share` body — the team to share the doc with.
#[derive(Debug, Deserialize)]
pub struct ShareDoc {
    pub team: String,
}

/// `POST /docs/{id}/share` — share a doc the caller owns with a team (gate 3, membership).
pub async fn share_doc(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ShareDoc>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::share_doc(&gw.node.store, &p, p.ws(), &id, &body.team)
        .await
        .map_err(asset_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /docs/{id}/link` body — the channel to link the doc into.
#[derive(Debug, Deserialize)]
pub struct LinkDoc {
    pub channel: String,
}

/// `POST /docs/{id}/link` — link a doc into a channel (its subs may then read it).
pub async fn link_doc(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<LinkDoc>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::link_doc(&gw.node.store, &p, p.ws(), &id, &body.channel)
        .await
        .map_err(asset_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /skills` body — create/overwrite a versioned skill.
#[derive(Debug, Deserialize)]
pub struct PutSkill {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub body: String,
}

/// `POST /skills` — upsert a versioned skill. Returns `{ id, version }`.
pub async fn put_skill(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<PutSkill>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let s = lb_host::put_skill(
        &gw.node.store,
        &p,
        p.ws(),
        &body.id,
        &body.version,
        &body.description,
        &body.body,
        gw.now(),
    )
    .await
    .map_err(asset_status)?;
    Ok(Json(json!({ "id": s.id, "version": s.version })))
}

/// `POST /skills/{id}/grant` — grant a skill to the workspace (so an agent may load it as substrate).
pub async fn grant_skill(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::grant_skill(&gw.node.store, &p, p.ws(), &id)
        .await
        .map_err(asset_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /skills/{id}` (optional `?version=`) — load a granted skill (latest, or a pinned version).
/// `403` if the workspace was never granted the skill (the grant gate), `404` if no such version.
#[derive(Debug, Deserialize)]
pub struct LoadSkillQuery {
    pub version: Option<String>,
}

pub async fn load_skill(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<LoadSkillQuery>,
) -> Result<Json<Skill>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let s = lb_host::load_skill(&gw.node.store, &p, p.ws(), &id, q.version.as_deref())
        .await
        .map_err(asset_status)?;
    Ok(Json(s))
}

/// Map an asset-gate outcome onto an HTTP status. `Denied` stays `403` (opaque — no existence
/// signal); `NotFound` (only returned to a caller who PASSED the gates) is `404`; a store fault is
/// `403`-opaque like every other gateway route.
fn asset_status(e: AssetError) -> (StatusCode, String) {
    match e {
        AssetError::NotFound => (StatusCode::NOT_FOUND, e.to_string()),
        other => (StatusCode::FORBIDDEN, other.to_string()),
    }
}
