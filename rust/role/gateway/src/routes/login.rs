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
    let mut claims = dev_claims(&principal, &req.workspace, gw.now(), SESSION_TTL_SECS);
    // Fold the DURABLE grant store into the token (authz-grants scope: the token is a cached
    // projection of `resolve_caps`). This is what lets an INSTALLED extension's tools reach a user
    // WITHOUT editing this login: install grants the ext's `mcp:<ext>.<tool>:call` caps to the
    // `workspace-admin` role, and any admin resolves them here. The `dev_claims` wildcard set stays
    // as the base (back-compat); resolved grants are unioned on top. Best-effort — a store hiccup
    // never fails the login (the base caps still mint a working dev session).
    // Grants are stored under the BARE user name (the seed + first-member bootstrap both
    // `grant_assign(Subject::User(sub.strip_prefix("user:")), …)`), so resolve with the bare name —
    // `resolve_caps` re-wraps it as `Subject::User`. Passing the `user:`-prefixed form would build
    // `Subject::User("user:ada")` and match zero grant rows (the bug that made an admin resolve to no
    // caps → every installed-extension page 403'd).
    let bare_user = principal.strip_prefix("user:").unwrap_or(&principal);
    if let Ok(resolved) = lb_host::resolve_caps(&gw.node.store, &req.workspace, bare_user).await {
        claims.caps.extend(resolved);
        claims.caps.sort();
        claims.caps.dedup();
    }

    // nav-reach scope: fold the subject's NAV-DERIVED reach caps (`reach:<surface>:view`) into the
    // token. Reach is now gated by caps like everything else — the surface entry routes require the
    // matching `reach:<surface>:view` (or the fallback wildcard) to OPEN a page. A subject given a
    // curated one-page nav reaches ONLY that page; a subject with no nav (fallback) gets `reach:*:view`
    // and reaches all (so a default member/admin is never locked out). This runs AFTER the grant fold
    // so the resolver strips items against the caller's FULL caps — reach can only ever name a surface
    // the caller could already reach (no-widening). Best-effort: a resolve hiccup falls back to the
    // permissive wildcard rather than locking the user out of their own session.
    let reach_principal = lb_auth::Principal::routed(
        principal.clone(),
        req.workspace.clone(),
        claims.caps.clone(),
    );
    let reach = match lb_host::nav_resolve(&gw.node, &reach_principal, &req.workspace).await {
        Ok(resolved) => lb_host::reach_caps(&resolved),
        // Never fail login on a nav-resolve error — degrade OPEN (reach all), same posture as the
        // grant fold above (a store hiccup never narrows a session below its floor).
        Err(_) => vec![lb_host::REACH_ALL.to_string()],
    };
    claims.caps.extend(reach);
    claims.caps.sort();
    claims.caps.dedup();

    let caps = claims.caps.clone();
    let token = mint(&gw.key, &claims);

    // Best-effort: make this workspace listable in the switcher. Never fails the login.
    let _ = lb_host::workspace_create(
        &gw.node.store,
        &verify_self(&gw, &token),
        &req.workspace,
        &req.workspace,
        gw.now(),
    )
    .await;

    Ok(Json(LoginReply {
        token,
        principal,
        workspace: req.workspace,
        caps,
    }))
}

/// Verify the just-minted token back into a principal (so the directory write runs under the real
/// session principal, with its `workspace.create` grant). The token was just signed by this key, so
/// this never fails — but going through `verify` keeps the principal construction in one place.
fn verify_self(gw: &Gateway, token: &str) -> lb_auth::Principal {
    lb_auth::verify(&gw.key, token, gw.now()).expect("self-minted token verifies")
}
