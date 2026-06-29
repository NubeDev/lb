//! Admin member-remove route — the missing destructive member verb over the gateway (admin-crud
//! scope). Mirrors `lb_host::remove_member`; gated server-side on `mcp:teams.manage:call`. The
//! collaboration `members` route keeps list/add; this adds remove.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};

use crate::session::authenticate;
use crate::state::Gateway;

/// `DELETE /teams/{team}/members/{user}` — remove a user from the team.
pub async fn remove_team_member(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((team, user)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::remove_member(&gw.node.store, &p, p.ws(), &team, &user)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
