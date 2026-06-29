//! Admin team routes — create / list / rename / delete over the gateway (admin-crud + authz-grants
//! scopes). `create`/`list` mirror `lb_host::teams_create`/`teams_list` (via the authz service);
//! `rename`/`delete` mirror the destructive teams service. Gated server-side on `mcp:teams.manage:call`
//! / `mcp:teams.list:call`. `delete` returns the cascade member-removed count for the consequence UI.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::Team;
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /admin/teams` — every team in the session's workspace.
pub async fn list_teams(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<Team>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let teams = lb_host::teams_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(forbid)?;
    Ok(Json(teams))
}

/// The `POST /admin/teams` body: the team id + display name.
#[derive(Debug, Deserialize)]
pub struct CreateTeam {
    pub team: String,
    pub name: String,
}

/// `POST /admin/teams` — create (or rename) a team.
pub async fn create_team(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<CreateTeam>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::teams_create(&gw.node.store, &p, p.ws(), &body.team, &body.name)
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// The `POST /admin/teams/{team}/rename` body.
#[derive(Debug, Deserialize)]
pub struct RenameTeam {
    pub name: String,
}

/// `POST /admin/teams/{team}/rename` — update the display name.
pub async fn rename_team(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(team): Path<String>,
    Json(body): Json<RenameTeam>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::teams_rename(&gw.node.store, &p, p.ws(), &team, &body.name)
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /admin/teams/{team}` — cascade-delete; returns the count of members removed.
pub async fn delete_team(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(team): Path<String>,
) -> Result<Json<usize>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let removed = lb_host::teams_delete(&gw.node.store, &p, p.ws(), &team)
        .await
        .map_err(forbid)?;
    Ok(Json(removed))
}

fn forbid(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, e.to_string())
}
