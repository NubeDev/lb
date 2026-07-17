//! `ANY /api/{*rest}` — resolve the session cookie to its bearer and dispatch **internally** to the
//! gateway's own `/{rest}` route (browser-session scope).
//!
//! **Why an internal dispatch and not a loopback hop.** One process, one router: no second listener,
//! no loopback port to firewall, no second TLS config, no extra network hop on a Pi. Crucially, it
//! also means `/api/*` can reach *exactly* the routes the bearer could and nothing else — the request
//! is handed to the same `Router` a CLI caller hits, so there is no separate route table to drift.
//!
//! **This seam grants nothing.** It attaches a bearer the caller already earned at login; every
//! downstream route runs its own `authenticate` + capability check unchanged. A session for workspace
//! A calling a verb scoped to workspace B is denied by the same membership/cap gate as the bearer
//! path — the seam is structurally incapable of widening authority, which is the property the
//! mandatory deny/isolation tests pin.
//!
//! **Not a general reverse proxy** (scope → Risks): `rest` is re-mounted on THIS router only. It can
//! never name an upstream, a scheme, or a host.

use axum::extract::{Request, State};
use axum::http::{header::AUTHORIZATION, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use tower::util::ServiceExt;

use super::cookie::{clear_cookie, read_sid};
use super::{csrf, store};
use crate::state::Gateway;

/// The `/api` prefix this seam is mounted under, stripped before internal dispatch.
const API_PREFIX: &str = "/api";

/// What the `/api/*` routes carry as axum state: the gateway, plus the **already-built** inner router
/// to dispatch into.
///
/// The inner router is built ONCE in `server::router` (with its state already applied) and cloned per
/// request — an axum `Router` is cheap to clone. Passing it as state, rather than hanging it off
/// `Gateway`, is what keeps this acyclic: `Gateway` knows nothing about a router, and the router that
/// serves `/api/*` holds a copy of the API router it forwards into.
#[derive(Clone)]
pub struct ApiState {
    pub gw: Gateway,
    pub inner: Router,
}

/// Resolve sid → JWT, attach it, and re-dispatch to the gateway's own router.
pub async fn forward(State(ApiState { gw, inner }): State<ApiState>, mut req: Request) -> Response {
    let Some(cfg) = gw.browser_session.as_ref().as_ref() else {
        // Unreachable: the route is only mounted when `Some`. Kept total rather than panicking.
        return StatusCode::NOT_FOUND.into_response();
    };

    // 1. CSRF gate FIRST — before any store read, before the cookie is trusted for anything. A
    //    cross-origin unsafe request is dead here even holding a perfectly valid cookie.
    if !csrf::is_allowed(req.method(), req.headers()) {
        return (StatusCode::FORBIDDEN, "cross-origin request rejected").into_response();
    }

    // 2. Resolve the session. An absent/unknown/expired sid is the SAME opaque 401 (no oracle) and
    //    never an anonymous pass-through to the inner route.
    let Some(sid) = read_sid(req.headers()) else {
        return (StatusCode::UNAUTHORIZED, "no session").into_response();
    };
    let row = match store::get(&gw.node.store, &sid, gw.now()).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            // Expired or forged: clear the dead cookie so the shell stops re-sending it.
            return (
                StatusCode::UNAUTHORIZED,
                [(
                    axum::http::header::SET_COOKIE,
                    clear_cookie(cfg.secure_cookie),
                )],
                "no session",
            )
                .into_response();
        }
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "session lookup failed").into_response();
        }
    };

    // 3. Rewrite `/api/{rest}` → `/{rest}`, preserving the query string.
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let stripped = path_and_query
        .strip_prefix(API_PREFIX)
        .filter(|s| s.starts_with('/'))
        .unwrap_or(path_and_query);
    let Ok(uri) = stripped.parse::<Uri>() else {
        return (StatusCode::BAD_REQUEST, "bad path").into_response();
    };
    *req.uri_mut() = uri;

    // 4. Attach the bearer the sid stands for. `insert` (not append) so a caller-supplied
    //    `Authorization` header can NEVER survive into the inner route — the cookie is the only
    //    credential this seam honours, and a browser must not be able to smuggle its own bearer.
    let Ok(value) = format!("Bearer {}", row.token).parse() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "bad session token").into_response();
    };
    req.headers_mut().insert(AUTHORIZATION, value);

    // 5. Dispatch into the very same router a bearer caller hits.
    match inner.oneshot(req).await {
        Ok(resp) => resp.into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "dispatch failed").into_response(),
    }
}
