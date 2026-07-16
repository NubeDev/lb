//! `POST /auth/select {workspace}` — the second half of the N-workspace login (email-login scope).
//! The caller presents the short-lived **select-token** from `/auth/login`'s N-branch as its bearer;
//! this route is the ONE acceptor of that token (every other route/verb refuses it — empty ws + empty
//! caps + the `ws-select` constraint). It re-verifies the person is still an effective member of the
//! chosen workspace (the roster in the client is a lens, not authority), then mints the full token.
//!
//! Security shape: the bearer must be a select-token (not a full token — a full token is refused here
//! so this route can't be a re-mint oracle for an already-authenticated session; that is `/auth/switch`).
//! Membership + disabled are re-checked server-side, so a select-token cannot reach a workspace the
//! person was removed from between login and select (→ `403`).

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;

use super::auth_reply::{AuthReply, WorkspaceRow};
use crate::session::{authenticate, is_select_token, mint_full_session};
use crate::state::Gateway;

/// The `/auth/select` request: which workspace to enter.
#[derive(Debug, Deserialize)]
pub struct AuthSelectRequest {
    pub workspace: String,
}

/// Exchange a valid select-token + a chosen workspace for the full session token.
pub async fn auth_select(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(req): Json<AuthSelectRequest>,
) -> Result<Json<AuthReply>, (StatusCode, String)> {
    // Verify the bearer (signature + expiry + revoke gates). A select-token verifies fine — it is a
    // signed JWT, just powerless. An expired select-token is a plain `401` here (the client fails soft
    // back to the login form).
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;

    // The bearer MUST be a select-token. A full token (or any non-select token) is refused — this
    // route only completes a login-in-progress, it does not re-mint an existing session.
    if !is_select_token(&principal) {
        return Err((StatusCode::UNAUTHORIZED, "not a select-token".to_string()));
    }
    let sub = principal.sub().to_string();
    let ws = req.workspace.trim().to_string();
    if ws.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "workspace required".to_string()));
    }
    let now = gw.now();

    // Re-verify membership from the CURRENT roster (never trust the client's list). A workspace the
    // person is not an effective member of — even with a valid select-token — is `403`.
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
