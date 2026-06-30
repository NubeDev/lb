//! Membership admin routes — the per-workspace roster over the gateway (global-identity scope).
//! Mirror `lb_host::membership_*` 1:1; authenticated by the session token; gated server-side on
//! `mcp:members.manage:call` in the session's workspace (ws from the token — a forged cross-workspace
//! add/remove is denied server-side). This is the Access console "People" tab source (decision #9).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::MembershipView;
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /admin/members` — the effective roster of the session's workspace.
pub async fn list_members_route(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<MembershipView>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let members = lb_host::membership_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(forbid)?;
    Ok(Json(members))
}

/// The `POST /admin/members` body: the global sub to add.
#[derive(Debug, Deserialize)]
pub struct AddMember {
    pub sub: String,
}

/// `POST /admin/members` — add `sub` to the session's workspace (grants `member`).
pub async fn add_member_route(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<AddMember>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::membership_add(&gw.node.store, &p, p.ws(), &body.sub, gw.now())
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /admin/members/{sub}` — remove `sub` (tombstone + revoke_subject + revoke_tokens).
/// Returns the count of grants revoked.
pub async fn remove_member_route(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(sub): Path<String>,
) -> Result<Json<usize>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let revoked = lb_host::membership_remove(&gw.node.store, &p, p.ws(), &sub)
        .await
        .map_err(forbid)?;
    Ok(Json(revoked))
}

fn forbid(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, e.to_string())
}
