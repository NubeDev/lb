//! `POST /auth/switch {workspace}` — password-less workspace switch (email-login scope). The caller
//! presents a **valid full session token**; this re-mints the SAME `sub` into another workspace they
//! are still an effective member of. This is what makes the workspace switcher real without the client
//! ever storing a password.
//!
//! Freshness is server-side: membership + disabled are re-checked in the TARGET at switch time (the
//! roster the client holds is a lens, not authority). An admin who removed the caller's membership an
//! hour ago makes this `403` — the client then refreshes its roster (the rubix-ai switcher's 403 path).
//! A select-token is refused here (it is for `/auth/select` only); the bearer must be a real session.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;

use super::auth_reply::{AuthReply, WorkspaceRow};
use crate::session::{authenticate, is_select_token, mint_full_session};
use crate::state::Gateway;

/// The `/auth/switch` request: which workspace to re-mint into.
#[derive(Debug, Deserialize)]
pub struct AuthSwitchRequest {
    pub workspace: String,
}

/// Re-mint the caller's session into `workspace` (same `sub`, target-workspace caps). No password.
pub async fn auth_switch(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(req): Json<AuthSwitchRequest>,
) -> Result<Json<AuthReply>, (StatusCode, String)> {
    // A valid full session token is required (verified: signature + expiry + revoke/run gates).
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    // A select-token cannot switch — it is powerless and belongs only to `/auth/select`.
    if is_select_token(&principal) {
        return Err((
            StatusCode::UNAUTHORIZED,
            "a select-token cannot switch".to_string(),
        ));
    }
    let sub = principal.sub().to_string();
    let ws = req.workspace.trim().to_string();
    if ws.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "workspace required".to_string()));
    }
    let now = gw.now();

    // Re-verify effective membership (and not-disabled) in the TARGET workspace — the switch cannot
    // reach a workspace the sub left or was disabled in. `login_workspaces` already filters both.
    let roster = lb_host::login_workspaces(&gw.node.store, &sub)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "membership resolution failed".to_string(),
            )
        })?;
    if !roster.iter().any(|w| w.ws == ws) {
        return Err((
            StatusCode::FORBIDDEN,
            "not a member of that workspace".to_string(),
        ));
    }

    let minted = mint_full_session(&gw.node, &gw.key, &sub, &ws, now).await;
    Ok(Json(AuthReply::session(
        minted.token,
        sub,
        ws,
        minted.caps,
        roster.into_iter().map(WorkspaceRow::from).collect(),
    )))
}
