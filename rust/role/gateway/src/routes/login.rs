//! `POST /login` — the session keystone (collaboration scope, slice 1). Issues a **real signed
//! token** the UI stores and sends on every subsequent request.
//!
//! The body is the dev-login `{user, workspace}` (no password yet — Non-goals); the gateway maps it
//! to a claim set (`session::dev_claims`) and `lb_auth::mint`s a signed token with the node key. From
//! here on every route `verify`s that token and derives the principal + workspace from it — the
//! workspace is the token's, never the request's (the hard wall, §7).
//!
//! As a convenience the login also **registers the workspace in the node directory** (best-effort) so
//! `workspace_list` shows it in the switcher — the demo's first login seeds the directory. A failure
//! there never fails the login (the token is the contract).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use lb_auth::mint;
use serde::{Deserialize, Serialize};

use crate::session::dev_claims;
use crate::state::Gateway;

/// The dev-login request: who, and into which workspace. A real credential (password / OIDC code)
/// lands here later behind the same seam.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub user: String,
    pub workspace: String,
}

/// The issued session: the signed token plus the resolved principal + workspace (so the UI need not
/// decode the token to render "logged in as …").
#[derive(Debug, Serialize)]
pub struct LoginReply {
    pub token: String,
    pub principal: String,
    pub workspace: String,
}

/// The dev session lifetime — long enough for a working session, short enough that a leaked token
/// expires. Config in a real deployment.
const SESSION_TTL_SECS: u64 = 60 * 60 * 12;

/// Mint a session token for the login request. Always `200` for the dev-login (any user); a real
/// credential check would `401` on bad credentials here.
pub async fn login(
    State(gw): State<Gateway>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginReply>, (StatusCode, String)> {
    if req.user.is_empty() || req.workspace.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "user and workspace required".into(),
        ));
    }
    let claims = dev_claims(&req.user, &req.workspace, gw.now, SESSION_TTL_SECS);
    let token = mint(&gw.key, &claims);

    // Best-effort: make this workspace listable in the switcher. Never fails the login.
    let _ = lb_host::workspace_create(
        &gw.node.store,
        &verify_self(&gw, &token),
        &req.workspace,
        &req.workspace,
        gw.now,
    )
    .await;

    Ok(Json(LoginReply {
        token,
        principal: req.user,
        workspace: req.workspace,
    }))
}

/// Verify the just-minted token back into a principal (so the directory write runs under the real
/// session principal, with its `workspace.create` grant). The token was just signed by this key, so
/// this never fails — but going through `verify` keeps the principal construction in one place.
fn verify_self(gw: &Gateway, token: &str) -> lb_auth::Principal {
    lb_auth::verify(&gw.key, token, gw.now).expect("self-minted token verifies")
}
