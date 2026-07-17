//! The `/api/auth/*` routes (browser-session scope): the verbs the two dev plugins already prove,
//! made real and store-backed.
//!
//! | Route | Behaviour |
//! |---|---|
//! | `POST /api/auth/login` | Call the REAL `auth_login` in-process; on a full session, store the JWT under a fresh sid and cookie the sid. |
//! | `POST /api/auth/select` | Complete the N>1 pick via the real `auth_select`; cookie a FRESH sid. |
//! | `POST /api/auth/switch` | Change workspace via the real `auth_switch`; cookie a FRESH sid. |
//! | `POST /api/auth/logout` | Delete the row; expire the cookie. |
//! | `GET /api/auth/session` | The current session's public facts, or `401`. Replaces each host's hand-rolled `/api/me/workspaces`. |
//!
//! **One implementation, not a second copy.** These call the SAME `routes::auth_*` handlers a bearer
//! client hits — in-process, not over a loopback hop — and then do the one extra thing the browser
//! needs: keep the token and hand back a cookie. Every credential rule (the uniform 401, the
//! timing-uniform argon2, the per-email rate limit, the 0/1/N branch) is inherited rather than
//! re-implemented. Re-deriving any of it here is exactly the third-and-fourth-copy failure the scope
//! exists to prevent.
//!
//! **The token never reaches the browser.** `AuthReply` carries the JWT; these routes move it into the
//! store and rebuild a reply from the public facts only. `token_never_reaches_browser` asserts this
//! across every route.
//!
//! **The 0/1/N branch is preserved, not flattened.** lb's `/auth/login` answers one of three ways
//! (email-login scope): 0 workspaces ⇒ `403`; 1 ⇒ a full token (the auto-skip); N>1 ⇒ a short-lived
//! *select-token* + a roster, and the client calls `/auth/select`. A seam handling only the
//! 1-workspace case would silently break every multi-workspace human. The select-token IS passed to
//! the browser: it is a 60s, workspace-less, pre-auth credential carrying no caps — not the fat
//! session JWT — and it is the only way the client can name its pick.
//!
//! **Rotation (scope → Risks: session fixation).** Login, select, and switch all mint a NEW sid and
//! delete the caller's old row: a pre-login or lower-privilege sid must never survive into an
//! authenticated or re-scoped session.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::Value;

use super::cookie::{clear_cookie, read_sid, set_cookie};
use super::forward::ApiState;
use super::sid::new_sid;
use super::store::{self, SessionRow};
use crate::routes::{AuthReply, WorkspaceRow};

/// The public fact set a shell is allowed to see — `AuthReply` minus the token.
///
/// `caps` is included because the shell folds it into its own coarse admin/member signal
/// (`roleFromCaps` in both dev plugins). That is a *convenience*, never the boundary: every verb is
/// re-checked server-side. Host domain logic (which personas exist, the default workspace) stays the
/// host's — lb returns facts, the shell folds them (scope → Non-goals).
#[derive(Debug, Serialize)]
pub struct SessionFacts {
    pub principal: String,
    pub workspace: String,
    pub caps: Vec<String>,
    /// The roster, so the shell can render its workspace switcher in the same round trip.
    pub workspaces: Vec<WorkspaceRow>,
}

/// The N>1 handoff: a select-token + the roster, no cookie and no session yet.
#[derive(Debug, Serialize)]
pub struct SelectNeeded {
    pub select_token: String,
    pub workspaces: Vec<WorkspaceRow>,
}

/// Turn a real `AuthReply` into a cookie + public facts, storing the token server-side.
///
/// This is the ONE place a minted token crosses into the session store, and the one place a
/// `Set-Cookie` is emitted for a login-shaped reply.
async fn establish(st: &ApiState, reply: AuthReply, old_sid: Option<String>) -> Response {
    let cfg = st
        .gw
        .browser_session
        .as_ref()
        .as_ref()
        .expect("route mounted only when Some");

    // The N-branch: no token, no cookie — hand the select-token back and wait for the pick.
    let (Some(token), Some(principal), Some(workspace), Some(caps)) =
        (reply.token, reply.principal, reply.workspace, reply.caps)
    else {
        return match reply.select_token {
            Some(select_token) => Json(SelectNeeded {
                select_token,
                workspaces: reply.workspaces,
            })
            .into_response(),
            // Neither a session nor a select: the inner handler's contract changed under us.
            None => (StatusCode::INTERNAL_SERVER_ERROR, "malformed auth reply").into_response(),
        };
    };

    // Privilege changed ⇒ the old sid dies (fixation). Done AFTER the credential check succeeded so a
    // failed login can't be used to log someone out.
    if let Some(old) = old_sid {
        let _ = store::remove(&st.gw.node.store, &old).await;
    }

    let sid = new_sid();
    let row = SessionRow {
        token,
        principal: principal.clone(),
        ws: workspace.clone(),
        expires_at: st.gw.now() + cfg.ttl_secs,
    };
    if store::put(&st.gw.node.store, &sid, &row).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "session store failed").into_response();
    }

    (
        [(
            axum::http::header::SET_COOKIE,
            set_cookie(&sid, cfg.secure_cookie),
        )],
        Json(SessionFacts {
            principal,
            workspace,
            caps,
            workspaces: reply.workspaces,
        }),
    )
        .into_response()
}

/// `POST /api/auth/login {email, password}` — the real `auth_login`, in-process.
pub async fn login(State(st): State<ApiState>, headers: HeaderMap, body: Json<Value>) -> Response {
    let old = read_sid(&headers);
    let req = match serde_json::from_value(body.0) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    match crate::routes::auth_login(State(st.gw.clone()), Json(req)).await {
        Ok(Json(reply)) => establish(&st, reply, old).await,
        Err((code, msg)) => (code, msg).into_response(),
    }
}

/// `POST /api/auth/select {select_token, workspace}` — complete the N>1 pick, cookie a fresh sid.
pub async fn select(State(st): State<ApiState>, headers: HeaderMap, body: Json<Value>) -> Response {
    let old = read_sid(&headers);
    let req = match serde_json::from_value(body.0) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    // `auth_select` authenticates the SELECT-TOKEN out of the request body/headers itself; the browser
    // holds no bearer, so the headers pass through as-is.
    match crate::routes::auth_select(State(st.gw.clone()), headers, Json(req)).await {
        Ok(Json(reply)) => establish(&st, reply, old).await,
        Err((code, msg)) => (code, msg).into_response(),
    }
}

/// `POST /api/auth/switch {workspace}` — re-mint into another workspace, cookie a fresh sid.
///
/// `auth_switch` requires a valid FULL token, which the browser does not hold — so the session's
/// stored token is attached here, exactly as `forward` does for every other route.
pub async fn switch(State(st): State<ApiState>, headers: HeaderMap, body: Json<Value>) -> Response {
    let Some(sid) = read_sid(&headers) else {
        return (StatusCode::UNAUTHORIZED, "no session").into_response();
    };
    let row = match store::get(&st.gw.node.store, &sid, st.gw.now()).await {
        Ok(Some(row)) => row,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "no session").into_response(),
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "session lookup failed").into_response()
        }
    };
    let req = match serde_json::from_value(body.0) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };

    let mut inner = HeaderMap::new();
    let Ok(value) = format!("Bearer {}", row.token).parse() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "bad session token").into_response();
    };
    inner.insert(axum::http::header::AUTHORIZATION, value);

    match crate::routes::auth_switch(State(st.gw.clone()), inner, Json(req)).await {
        Ok(Json(reply)) => establish(&st, reply, Some(sid)).await,
        Err((code, msg)) => (code, msg).into_response(),
    }
}

/// `POST /api/auth/logout` — delete the row, expire the cookie. Idempotent: no session is still a
/// clean `200` (a logout that 401s is a UX trap).
pub async fn logout(State(st): State<ApiState>, headers: HeaderMap) -> Response {
    let cfg = st
        .gw
        .browser_session
        .as_ref()
        .as_ref()
        .expect("route mounted only when Some");
    if let Some(sid) = read_sid(&headers) {
        let _ = store::remove(&st.gw.node.store, &sid).await;
    }
    (
        [(
            axum::http::header::SET_COOKIE,
            clear_cookie(cfg.secure_cookie),
        )],
        Json(serde_json::json!({})),
    )
        .into_response()
}

/// `GET /api/auth/session` — the current session's public facts, or `401`.
///
/// Caps are read off the STORED TOKEN via the node key, never a cached field, so a re-minted session
/// reports the truth.
pub async fn session(State(st): State<ApiState>, headers: HeaderMap) -> Response {
    let Some(sid) = read_sid(&headers) else {
        return (StatusCode::UNAUTHORIZED, "no session").into_response();
    };
    match store::get(&st.gw.node.store, &sid, st.gw.now()).await {
        Ok(Some(row)) => {
            let caps = match lb_auth::verify(&st.gw.key, &row.token, st.gw.now()) {
                Ok(p) => p.caps().to_vec(),
                // The row outlived its token: report no session rather than serve stale authority.
                Err(_) => return (StatusCode::UNAUTHORIZED, "no session").into_response(),
            };
            Json(SessionFacts {
                principal: row.principal,
                workspace: row.ws,
                caps,
                workspaces: Vec::new(),
            })
            .into_response()
        }
        Ok(None) => (StatusCode::UNAUTHORIZED, "no session").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "session lookup failed").into_response(),
    }
}
