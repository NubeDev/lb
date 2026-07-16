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
use serde::{Deserialize, Serialize};

use crate::session::mint_full_session;
use crate::state::Gateway;

/// The login request: who, into which workspace, and the credential proving it (login-hardening
/// scope). `secret` is the password checked by the node's `CredentialCheck` before minting. It is
/// OPTIONAL on the wire: a `DevTrustAny` node (dev/CI, `LB_DEV_LOGIN`) ignores it (password-less);
/// a `PasswordHash` node requires it (an empty/absent secret → `401`). A future OIDC impl reads a
/// `code` here behind the same seam.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub user: String,
    pub workspace: String,
    /// The login secret (password). Optional so a dev-login body may omit it; the credential check
    /// decides whether its absence is allowed.
    #[serde(default)]
    pub secret: String,
}

/// The issued session: the signed token plus the resolved principal + workspace (so the UI need not
/// decode the token to render "logged in as …").
#[derive(Debug, Serialize)]
pub struct LoginReply {
    pub token: String,
    pub principal: String,
    pub workspace: String,
    /// The capabilities the token carries — surfaced so the UI can cap-gate which admin controls
    /// it *shows*. This is a CONVENIENCE only: the gateway re-checks every verb server-side (the UI
    /// gate is never the security boundary — admin-console scope). Hiding a control the caller lacks
    /// avoids dead buttons; a forged call is still denied at the route.
    pub caps: Vec<String>,
}

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
    // Canonicalize the login handle to the `user:<name>` principal the whole identity model keys on
    // (the token `sub`, the `membership` row, and every `created_by` use this form; the seed writes
    // `LB_SEED_USER=user:ada`). The dev-login accepts a bare handle for convenience — a user typing
    // `ada` means the identity `user:ada`, NOT a distinct principal literally named "ada". Without
    // this, `ada` minted a token for a stranger and `membership_login_resolve` returned NotAMember
    // against a workspace already seeded with `user:ada` ("not a member of any workspace" on the
    // persistent `make dev` node, though it worked on an empty in-memory node because the stranger
    // bootstrapped the empty ws). Idempotent: an already-prefixed `user:ada` is unchanged. The grant
    // resolution below re-strips the prefix (grants are stored bare — see there).
    let principal = if req.user.starts_with("user:") {
        req.user.clone()
    } else {
        format!("user:{}", req.user)
    };
    // admin-crud: a disabled/deleted user record refuses to mint a session (disable bites login).
    // An un-administered workspace (no user record) still mints — the dev-login auto-seeds.
    lb_host::user_login_check(&gw.node.store, &req.workspace, &principal)
        .await
        .map_err(|_| (StatusCode::FORBIDDEN, "user is disabled".into()))?;
    // global-identity: membership resolves login. An effective member mints; an empty workspace
    // bootstraps the requester as workspace-admin (decision #3); a workspace that has members but not
    // this sub refuses with "not a member" (decision #4). Identity is lazy-created on first touch.
    lb_host::membership_login_resolve(&gw.node.store, &req.workspace, &principal, gw.now())
        .await
        .map_err(|_| {
            (
                StatusCode::FORBIDDEN,
                "not a member of any workspace".into(),
            )
        })?;
    // Credential check (login-hardening scope, change 2): PROVE identity before minting. A
    // `PasswordHash` node checks argon2 against the stored `(ws, user)` credential; a `DevTrustAny`
    // node (opt-in via `LB_DEV_LOGIN`) passes password-less. A bad/absent secret is an opaque `401`
    // with NO token — authenticity is decided before authority, and before any claim is built. Runs
    // after membership resolution so the credential is verified for a real member of a real ws.
    gw.credential_check
        .verify(&gw.node, &req.workspace, &principal, &req.secret)
        .await
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "invalid or missing credential".into(),
            )
        })?;
    // Mint through the ONE shared role-correct path (email-login scope) — the same issuance the
    // `/auth/*` routes use: viewer floor ∪ resolved grants ∪ nav-reach, best-effort directory register.
    let minted = mint_full_session(&gw.node, &gw.key, &principal, &req.workspace, gw.now()).await;

    Ok(Json(LoginReply {
        token: minted.token,
        principal,
        workspace: req.workspace,
        caps: minted.caps,
    }))
}
