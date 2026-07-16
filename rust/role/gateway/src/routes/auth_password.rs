//! `POST /auth/password {old, new}` — self-service password change (email-login scope). A person
//! authenticated with a valid full token changes THEIR OWN global password. Not admin-gated: the
//! authorization is "you hold a valid session for this `sub`" (verified here) PLUS "you know the
//! current password" (verified in `identity_change_password`). It can only change the caller's own
//! credential — `sub` is taken from the verified token, never the body.
//!
//! A wrong/absent current password is `401` (never reveals which). The admin "set a first password /
//! reset someone else's" path is the separate `identity.set_password` verb; this route requires the old.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// The `/auth/password` request: the current + new password. Neither is ever logged/echoed.
#[derive(Debug, Deserialize)]
pub struct AuthPasswordRequest {
    #[serde(default)]
    pub old: String,
    #[serde(default)]
    pub new: String,
}

/// Change the caller's own global password.
pub async fn auth_password(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(req): Json<AuthPasswordRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // A valid full session is required; the `sub` to change is the token's own subject (never the body).
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let sub = principal.sub().to_string();

    lb_host::identity_change_password(&gw.node.store, &sub, &req.old, &req.new, gw.now())
        .await
        .map_err(|e| match e {
            // A wrong/absent current password is an opaque 401 (authenticity, no oracle).
            lb_host::IdentityCredentialError::BadOldSecret => {
                (StatusCode::UNAUTHORIZED, "invalid credentials".to_string())
            }
            lb_host::IdentityCredentialError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
            lb_host::IdentityCredentialError::Denied => {
                (StatusCode::FORBIDDEN, "denied".to_string())
            }
            other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
        })?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
