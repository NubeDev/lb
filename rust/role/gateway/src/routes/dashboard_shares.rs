//! Dashboard SHARING routes — the three verbs that move a board's audience, split out of
//! `dashboard.rs` so that file stays inside the FILE-LAYOUT ceiling (folder-of-verbs, not a growing
//! `dashboard.rs`). Same contract as its sibling: each route mirrors a `lb_host::dashboard_*` verb
//! 1:1 and re-runs the host's three gates server-side. All three are gated `mcp:dashboard.share:call`
//! and are owner-only INSIDE the host verb — the route never decides that itself.
//!
//! The trio mirrors the `nav.*` shape deliberately (`share` / `unshare` / `list_shares`), because a
//! board's audience and a nav's audience are the same problem and had drifted: dashboards shipped
//! `share` with **no way to remove an edge at any layer**, and setting a board back to `private`
//! leaves the edge live for the next `team` flip.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::dashboard::{parse_visibility, status};
use crate::session::authenticate;
use crate::state::Gateway;

/// `POST /dashboards/{id}/share` body — set visibility (`private|team|workspace`) + optional team.
#[derive(Debug, Deserialize)]
pub struct ShareDashboard {
    pub visibility: String,
    #[serde(default)]
    pub team: Option<String>,
}

/// `POST /dashboards/{id}/share` — set a dashboard's visibility / write the S4 share edge. Gated
/// `dashboard.share`; owner-only. Returns the updated dashboard.
pub async fn share_dashboard(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ShareDashboard>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let visibility = parse_visibility(&body.visibility).ok_or((
        StatusCode::BAD_REQUEST,
        format!("bad visibility: {}", body.visibility),
    ))?;
    let d = lb_host::dashboard_share(
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
    Ok(Json(serde_json::to_value(d).unwrap_or(Value::Null)))
}

/// `POST /dashboards/{id}/unshare` body — the team whose share edge to revoke.
#[derive(Debug, Deserialize)]
pub struct UnshareDashboard {
    pub team: String,
}

/// `POST /dashboards/{id}/unshare` — revoke one team share. Gated `dashboard.share`; owner-only.
/// The mirror of `POST /navs/{id}/unshare`; without it a share edge could never be removed.
pub async fn unshare_dashboard(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UnshareDashboard>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let d = lb_host::dashboard_unshare(&gw.node.store, &p, p.ws(), &id, &body.team, gw.now())
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(d).unwrap_or(Value::Null)))
}

/// `GET /dashboards/{id}/shares` — enumerate the live team shares. Gated `dashboard.share`;
/// owner-only. The mirror of `GET /navs/{id}/shares`; the onboarding access preview needs it to say
/// truthfully whether a person can open the boards a nav names.
pub async fn list_shares_dashboard(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let teams = lb_host::dashboard_list_shares(&gw.node.store, &p, p.ws(), &id)
        .await
        .map_err(status)?;
    Ok(Json(json!({ "teams": teams })))
}
