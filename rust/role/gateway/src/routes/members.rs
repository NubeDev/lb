//! `GET /teams/{team}/members` + `POST /teams/{team}/members` — the members/teams UI's list + add
//! (collaboration scope, slice 3). Mirrors `lb_host::list_members` / `add_team_member`. Authenticated
//! by the session token; gated by `mcp:members.<verb>:call`.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /teams/{team}/members` — every user in the team.
pub async fn list_team_members(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(team): Path<String>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let members = lb_host::list_members(&gw.node.store, &principal, principal.ws(), &team)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(members))
}

/// The `POST /teams/{team}/members` body: the user to add.
#[derive(Debug, Deserialize)]
pub struct AddMember {
    pub user: String,
}

/// `POST /teams/{team}/members` — add a user to the team.
pub async fn add_team_member(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(team): Path<String>,
    Json(body): Json<AddMember>,
) -> Result<StatusCode, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::add_team_member(
        &gw.node.store,
        &principal,
        principal.ws(),
        &team,
        &body.user,
    )
    .await
    .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
