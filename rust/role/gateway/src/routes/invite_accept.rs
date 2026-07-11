//! The pre-auth invite accept route (invites scope) — `POST /public/invite/accept`. The one
//! unauthenticated surface that creates a session: the caller presents the invite token (not a
//! session), and the accept chain runs the atomic onboarding (verify → identity → credential →
//! membership → grants → mint). The gateway's signing key mints the session token.
//!
//! This is the THIRD public route (besides `/login` and `/hooks`) — it is not behind the session
//! authenticate layer. Rate-limiting is the gateway's concern (the public route ships rate-limited
//! from day one per the scope's risk note).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::state::Gateway;

/// The `POST /public/invite/accept` body.
#[derive(Debug, Deserialize)]
pub struct AcceptInviteRequest {
    /// The raw invite token (`lbi_…`).
    pub token: String,
    /// The workspace to join (must match the invite's workspace).
    pub workspace: String,
    /// The new password to set.
    pub secret: String,
    /// Required if the identity already has a credential (prevents email-match takeover).
    #[serde(default)]
    pub current_secret: Option<String>,
}

/// The accept reply — a session token + principal info (same shape as `LoginReply`).
#[derive(Debug, Serialize)]
pub struct AcceptInviteReply {
    pub token: String,
    pub principal: String,
    pub workspace: String,
    pub caps: Vec<String>,
}

/// `POST /public/invite/accept` — accept an invite, creating the identity + membership + grants
/// and minting a session token. Pre-auth: no session token required.
pub async fn accept_invite(
    State(gw): State<Gateway>,
    Json(req): Json<AcceptInviteRequest>,
) -> Result<Json<AcceptInviteReply>, (StatusCode, String)> {
    let now = gw.now();
    let accepted = lb_host::invite_accept(
        &gw.node.store,
        gw.key.as_ref(),
        &req.workspace,
        &req.token,
        &req.secret,
        req.current_secret.as_deref(),
        now,
    )
    .await
    .map_err(|e| {
        let code = match &e {
            lb_host::InviteError::Denied => StatusCode::FORBIDDEN,
            lb_host::InviteError::NotFound | lb_host::InviteError::BadToken => {
                StatusCode::NOT_FOUND
            }
            lb_host::InviteError::Expired
            | lb_host::InviteError::AlreadyAccepted
            | lb_host::InviteError::Revoked => StatusCode::GONE,
            lb_host::InviteError::IdentityExists(_) => StatusCode::CONFLICT,
            lb_host::InviteError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (code, e.to_string())
    })?;

    Ok(Json(AcceptInviteReply {
        token: accepted.token,
        principal: accepted.sub,
        workspace: accepted.workspace,
        caps: accepted.caps,
    }))
}
