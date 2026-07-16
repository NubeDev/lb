//! `POST /auth/login {email, password}` — the Slack-style human front door (email-login scope). The
//! ONE authenticate-then-choose entry: verify the person's GLOBAL credential, enumerate the
//! workspaces they may enter, and branch 0/1/N:
//!   - **0** → `403 "not a member of any workspace"`, no token (global-identity decision #4).
//!   - **1** → the full workspace token immediately (the auto-skip — the client never sees a picker).
//!   - **N** → a short-lived select-token + the roster; the client picks and calls `/auth/select`.
//!
//! Unauthenticated by nature (it ISSUES the token). Hardened by: a uniform `401 "invalid credentials"`
//! whether the email is unknown OR the password is wrong (no account-enumeration oracle), a
//! timing-uniform credential verify (argon2 burned even on an unknown email — `global_credential_verify`),
//! and a per-email failure rate-limit (`rate_limit::auth_login_*`). The workspace is chosen AFTER
//! authentication; the minted token carries exactly one `ws` (the hard wall, unchanged).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use lb_host::fold_email;
use serde::Deserialize;

use super::auth_reply::{AuthReply, WorkspaceRow};
use super::rate_limit::{auth_login_allowed, auth_login_record_failure};
use crate::session::mint_full_session;
use crate::state::Gateway;

/// The `/auth/login` request: the human's email + password. Nothing else — no `user:` prefix, no
/// workspace name ever crosses this wire (the whole point of the scope).
#[derive(Debug, Deserialize)]
pub struct AuthLoginRequest {
    pub email: String,
    #[serde(default)]
    pub password: String,
}

/// The uniform credential-failure response — identical for an unknown email and a wrong password (no
/// oracle). `401`, one body.
fn invalid_credentials() -> (StatusCode, String) {
    (StatusCode::UNAUTHORIZED, "invalid credentials".to_string())
}

/// Authenticate `{email, password}` globally, then branch 0/1/N on the person's workspaces.
pub async fn auth_login(
    State(gw): State<Gateway>,
    Json(req): Json<AuthLoginRequest>,
) -> Result<Json<AuthReply>, (StatusCode, String)> {
    let folded = fold_email(&req.email);
    if folded.is_empty() {
        return Err(invalid_credentials());
    }
    let now = gw.now();

    // Per-email failure rate limit (resolved open question: 10 failures / 15 min). Checked BEFORE any
    // work so a locked-out email cannot keep burning argon2. A locked email is `429`.
    if !auth_login_allowed(&folded, now) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "too many attempts — retry later".to_string(),
        ));
    }

    // Resolve email → sub. An UNKNOWN email does NOT early-return: we still run the credential check
    // (against a dummy hash, inside `global_credential_verify`) so the timing matches a wrong password,
    // then fail with the uniform 401. That means we verify a placeholder sub when the email is unknown.
    let sub = lb_host::identity_by_email(&gw.node.store, &folded)
        .await
        .map_err(|_| invalid_credentials())?;
    let sub_for_verify = sub
        .clone()
        .unwrap_or_else(|| "user:__unknown__".to_string());

    // Verify the global credential. `GlobalPasswordHash` burns argon2 even for an unknown/absent
    // credential (timing-uniform); `GlobalDevTrustAny` (LB_DEV_LOGIN) passes password-less.
    let credential_ok = gw
        .global_credential_check
        .verify(&gw.node, &sub_for_verify, &req.password)
        .await
        .is_ok();

    // A known email is required to have a real sub; under DevTrustAny an unknown email still has no
    // membership, so the enumeration below returns empty and yields the same 403 branch as decision #4.
    let Some(sub) = sub else {
        // Unknown email. Under production (`GlobalPasswordHash`) the verify already failed; under
        // `DevTrustAny` it "passed" but there is no identity — either way, a login for a nonexistent
        // person is the uniform credential failure (never "no such email").
        auth_login_record_failure(&folded, now);
        return Err(invalid_credentials());
    };

    if !credential_ok {
        auth_login_record_failure(&folded, now);
        return Err(invalid_credentials());
    }

    // Credential proven. Enumerate the workspaces this person may enter (effective member AND not
    // disabled there). This is un-gated (pre-principal) — `login_workspaces`.
    let roster = lb_host::login_workspaces(&gw.node.store, &sub)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "membership resolution failed".to_string(),
            )
        })?;

    match roster.len() {
        0 => Err((
            StatusCode::FORBIDDEN,
            "not a member of any workspace".to_string(),
        )),
        1 => {
            // The auto-skip: one workspace → the full token immediately.
            let ws = roster[0].ws.clone();
            let minted = mint_full_session(&gw.node, &gw.key, &sub, &ws, now).await;
            Ok(Json(AuthReply::session(
                minted.token,
                sub,
                ws,
                minted.caps,
                roster.into_iter().map(WorkspaceRow::from).collect(),
            )))
        }
        _ => {
            // N>1: a select-token + the roster, no full token. The client picks → `/auth/select`.
            let select = crate::session::mint_select_token(&gw.key, &sub, now);
            Ok(Json(AuthReply::select(
                select,
                roster.into_iter().map(WorkspaceRow::from).collect(),
            )))
        }
    }
}
