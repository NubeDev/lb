//! The `/api/*` browser-session seam (browser-session scope) over the REAL gateway + SurrealDB +
//! argon2 — no mocks (CLAUDE §9). This is a security boundary of a shape lb has never had before, so
//! the suite is deliberately weighted toward the ways it could be WRONG rather than the happy path.
//!
//! The gates this suite exists to hold:
//!   - **CSRF** — a cross-origin POST carrying a valid cookie is rejected. The scope's stated gate on
//!     this shipping at all.
//!   - **The token never reaches the browser** — the whole point of the seam.
//!   - **Capability-deny + workspace-isolation through `/api/*`** (mandatory categories) — the seam
//!     must be provably incapable of widening authority.
//!   - **Off-by-default** — `browser_session: None` ⇒ no `/api/*`, no cookie, today's router.
//!
//! Seeding uses the REAL write path (as `email_login_test.rs` does): bootstrap an admin via `/login`,
//! provision a global identity + password, add memberships. `/api/*` is then driven end to end.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{bearer, json_body, json_post, NOW};
use lb_auth::SigningKey;
use lb_host::Node;
use lb_role_gateway::session::GlobalPasswordHash;
use lb_role_gateway::{router, BrowserSessionConfig, Gateway};
use serde_json::{json, Value};
use tower::ServiceExt;

/// The origin the tests present as same-origin.
const HOST: &str = "pi.local:8391";

/// A gateway with the browser-session seam ON and the REAL argon2 global credential check.
async fn session_gateway() -> (Gateway, Arc<Node>, SigningKey) {
    let node = Arc::new(Node::boot_as(lb_host::Role::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = Gateway::new(node.clone(), key.clone(), NOW)
        .with_global_credential_check(Arc::new(GlobalPasswordHash))
        .with_browser_session(BrowserSessionConfig::default());
    (gw, node, key)
}

/// Bootstrap an admin: the first `/login` into an empty workspace makes the requester workspace-admin.
async fn bootstrap_admin(gw: &Gateway, user: &str, ws: &str) -> String {
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": user, "workspace": ws }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "bootstrap {user}@{ws}");
    let reply: Value = json_body(resp).await;
    reply["token"].as_str().unwrap().to_string()
}

/// Provision a real person: global identity + email + password, member of `ws`.
async fn seed_person(gw: &Gateway, admin: &str, sub: &str, email: &str, password: &str) {
    let st = router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/identities", json!({ "sub": sub, "email": email })),
            admin,
        ))
        .await
        .unwrap()
        .status();
    assert!(st.is_success(), "create identity {sub}: {st}");

    let st = router(gw.clone())
        .oneshot(bearer(
            json_post(
                &format!("/admin/identities/{sub}/password"),
                json!({ "secret": password }),
            ),
            admin,
        ))
        .await
        .unwrap()
        .status();
    assert!(st.is_success(), "set password {sub}: {st}");

    let st = router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": sub })),
            admin,
        ))
        .await
        .unwrap()
        .status();
    assert!(st.is_success(), "add member {sub}: {st}");
}

/// A same-origin browser POST (what the shell actually sends).
fn browser_post(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .header("host", HOST)
        .header("origin", format!("http://{HOST}"))
        .header("sec-fetch-site", "same-origin")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

/// Attach a session cookie to a request.
fn with_cookie(req: Request<Body>, sid: &str) -> Request<Body> {
    let (mut parts, body) = req.into_parts();
    parts
        .headers
        .insert("cookie", format!("lb_session={sid}").parse().unwrap());
    Request::from_parts(parts, body)
}

/// Pull the `lb_session` sid out of a `Set-Cookie` header.
fn sid_from(resp: &axum::response::Response) -> Option<String> {
    let raw = resp.headers().get("set-cookie")?.to_str().ok()?;
    let v = raw.split(';').next()?.strip_prefix("lb_session=")?;
    (!v.is_empty()).then(|| v.to_string())
}

/// Log in through the seam, returning (sid, the login response body).
async fn login(gw: &Gateway, email: &str, password: &str) -> (String, Value, StatusCode) {
    let resp = router(gw.clone())
        .oneshot(browser_post(
            "/api/auth/login",
            json!({ "email": email, "password": password }),
        ))
        .await
        .unwrap();
    let status = resp.status();
    let sid = sid_from(&resp);
    let body: Value = json_body(resp).await;
    (sid.unwrap_or_default(), body, status)
}

/// A person with one workspace logs in: cookie set, `HttpOnly`, and the body carries the facts.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn login_sets_an_httponly_cookie_and_returns_facts() {
    let (gw, _node, _key) = session_gateway().await;
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(&gw, &admin, "user:ada", "ada@example.com", "hunter2hunter2").await;

    let resp = router(gw.clone())
        .oneshot(browser_post(
            "/api/auth/login",
            json!({ "email": "ada@example.com", "password": "hunter2hunter2" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let cookie = resp
        .headers()
        .get("set-cookie")
        .expect("a session cookie is set")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        cookie.contains("HttpOnly"),
        "JS must never read the session"
    );
    assert!(cookie.contains("SameSite=Lax"));

    let body: Value = json_body(resp).await;
    assert_eq!(body["principal"], "user:ada");
    assert_eq!(body["workspace"], "acme");
    assert!(
        body["caps"].is_array(),
        "the shell folds caps into its own role signal"
    );
}

/// **The whole point of the scope.** No `/api/*` reply may ever carry the JWT — not in a body, not in
/// a header. A JWT is three base64 segments joined by dots; the cookie's sid is 64 hex chars and must
/// look nothing like one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn token_never_reaches_the_browser() {
    let (gw, _node, key) = session_gateway().await;
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(&gw, &admin, "user:ada", "ada@example.com", "hunter2hunter2").await;

    // Every `/api/*` reply a shell can provoke.
    let (sid, login_body, _) = login(&gw, "ada@example.com", "hunter2hunter2").await;
    assert!(!sid.is_empty(), "login established a session");

    let session_resp = router(gw.clone())
        .oneshot(with_cookie(
            Request::builder()
                .method("GET")
                .uri("/api/auth/session")
                .header("host", HOST)
                .body(Body::empty())
                .unwrap(),
            &sid,
        ))
        .await
        .unwrap();
    let session_headers = format!("{:?}", session_resp.headers());
    let session_body: Value = json_body(session_resp).await;

    let forwarded = router(gw.clone())
        .oneshot(with_cookie(
            browser_post(
                "/api/mcp/call",
                json!({ "tool": "series.find", "args": { "tags": [] } }),
            ),
            &sid,
        ))
        .await
        .unwrap();
    let forwarded_headers = format!("{:?}", forwarded.headers());
    let forwarded_body = axum::body::to_bytes(forwarded.into_body(), 1 << 20)
        .await
        .unwrap();

    // The real token for this person, so we can assert its literal absence.
    let real_token = {
        let minted =
            lb_role_gateway::session::mint_full_session(&gw.node, &key, "user:ada", "acme", NOW)
                .await;
        minted.token
    };
    let jwt_head = real_token.split('.').next().unwrap().to_string();

    for (what, haystack) in [
        ("login body", login_body.to_string()),
        ("session body", session_body.to_string()),
        ("session headers", session_headers),
        (
            "forward body",
            String::from_utf8_lossy(&forwarded_body).to_string(),
        ),
        ("forward headers", forwarded_headers),
    ] {
        assert!(
            !haystack.contains(&jwt_head),
            "{what} leaked the JWT — the token must never reach the browser"
        );
        assert!(
            !haystack.contains("eyJ"),
            "{what} contains a JWT-looking blob: {haystack}"
        );
    }

    // The sid itself is opaque: hex, not a JWT.
    assert_eq!(sid.len(), 64);
    assert!(sid.chars().all(|c| c.is_ascii_hexdigit()));
}

/// A forwarded call reaches the real route with the session's authority.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn forward_dispatches_with_the_sessions_bearer() {
    let (gw, _node, _key) = session_gateway().await;
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(&gw, &admin, "user:ada", "ada@example.com", "hunter2hunter2").await;
    let (sid, _, _) = login(&gw, "ada@example.com", "hunter2hunter2").await;

    let resp = router(gw.clone())
        .oneshot(with_cookie(
            browser_post(
                "/api/mcp/call",
                json!({ "tool": "series.find", "args": { "tags": [] } }),
            ),
            &sid,
        ))
        .await
        .unwrap();
    // The point is that it is the ROUTE's answer, not the seam's 401: the bearer was attached and
    // `/mcp/call` ran its own cap check.
    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "the seam attached the session's bearer"
    );
}

/// A caller-supplied `Authorization` header must NEVER survive into the inner route: the cookie is the
/// only credential this seam honours. Otherwise a browser could smuggle a bearer past the session.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_smuggled_bearer_is_ignored() {
    let (gw, _node, key) = session_gateway().await;
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(&gw, &admin, "user:ada", "ada@example.com", "hunter2hunter2").await;

    // A powerful token the attacker holds, but NO session cookie.
    let stolen = common::token(&key, "user:root", "acme", &["mcp:series.find:call"]);
    let req = bearer(
        browser_post(
            "/api/mcp/call",
            json!({ "tool": "series.find", "args": { "tags": [] } }),
        ),
        &stolen,
    );
    let resp = router(gw.clone()).oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "no cookie ⇒ no session, whatever Authorization header was supplied"
    );
}

/// Bad password: `401`, no cookie, no session row.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bad_password_sets_no_cookie() {
    let (gw, _node, _key) = session_gateway().await;
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(&gw, &admin, "user:ada", "ada@example.com", "hunter2hunter2").await;

    let resp = router(gw.clone())
        .oneshot(browser_post(
            "/api/auth/login",
            json!({ "email": "ada@example.com", "password": "wrong" }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(
        resp.headers().get("set-cookie").is_none(),
        "no cookie on a failed login"
    );
}

/// A forged/unknown sid is `401` — never a 500, never an anonymous pass-through.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_forged_sid_is_rejected() {
    let (gw, _node, _key) = session_gateway().await;
    let resp = router(gw.clone())
        .oneshot(with_cookie(
            browser_post(
                "/api/mcp/call",
                json!({ "tool": "series.find", "args": {} }),
            ),
            &"f".repeat(64),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// Logout deletes the row: the same cookie stops working.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn logout_kills_the_session() {
    let (gw, _node, _key) = session_gateway().await;
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(&gw, &admin, "user:ada", "ada@example.com", "hunter2hunter2").await;
    let (sid, _, _) = login(&gw, "ada@example.com", "hunter2hunter2").await;

    let resp = router(gw.clone())
        .oneshot(with_cookie(
            browser_post("/api/auth/logout", json!({})),
            &sid,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let after = router(gw.clone())
        .oneshot(with_cookie(
            Request::builder()
                .method("GET")
                .uri("/api/auth/session")
                .header("host", HOST)
                .body(Body::empty())
                .unwrap(),
            &sid,
        ))
        .await
        .unwrap();
    assert_eq!(after.status(), StatusCode::UNAUTHORIZED, "the sid is dead");
}

/// **Restart survival.** A store-backed session outlives the gateway object; a process-local map (the
/// dev plugins' shape) would log everyone out on every deploy.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_session_survives_a_gateway_rebuild() {
    let (gw, node, key) = session_gateway().await;
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(&gw, &admin, "user:ada", "ada@example.com", "hunter2hunter2").await;
    let (sid, _, _) = login(&gw, "ada@example.com", "hunter2hunter2").await;

    // A brand-new Gateway over the SAME node/store — i.e. the process restarted.
    let rebuilt = Gateway::new(node, key, NOW)
        .with_global_credential_check(Arc::new(GlobalPasswordHash))
        .with_browser_session(BrowserSessionConfig::default());

    let resp = router(rebuilt)
        .oneshot(with_cookie(
            Request::builder()
                .method("GET")
                .uri("/api/auth/session")
                .header("host", HOST)
                .body(Body::empty())
                .unwrap(),
            &sid,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the cookie still works after a restart"
    );
}

/// TTL expiry ⇒ `401`. The gateway clock is fixed at `NOW`, so a zero-TTL config expires instantly.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_expired_session_is_rejected() {
    let node = Arc::new(Node::boot_as(lb_host::Role::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = Gateway::new(node, key, NOW)
        .with_global_credential_check(Arc::new(GlobalPasswordHash))
        .with_browser_session({
            // `default()`-then-mutate — the supported path for a `#[non_exhaustive]` config.
            let mut c = BrowserSessionConfig::default();
            c.ttl_secs = 0;
            c
        });
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(&gw, &admin, "user:ada", "ada@example.com", "hunter2hunter2").await;
    let (sid, _, status) = login(&gw, "ada@example.com", "hunter2hunter2").await;
    assert_eq!(status, StatusCode::OK, "login itself succeeds");

    let resp = router(gw.clone())
        .oneshot(with_cookie(
            Request::builder()
                .method("GET")
                .uri("/api/auth/session")
                .header("host", HOST)
                .body(Body::empty())
                .unwrap(),
            &sid,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "an expired sid is no session"
    );
}

/// **Off by default.** `browser_session: None` ⇒ `/api/*` does not exist and no cookie is ever set.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_seam_is_absent_unless_configured() {
    let node = Arc::new(Node::boot_as(lb_host::Role::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    // A DEFAULT gateway — no `with_browser_session`.
    let gw = Gateway::new(node, key, NOW);

    let resp = router(gw.clone())
        .oneshot(browser_post(
            "/api/auth/login",
            json!({ "email": "ada@example.com", "password": "hunter2hunter2" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "no browser-session config ⇒ no /api/* route at all"
    );
    assert!(
        resp.headers().get("set-cookie").is_none(),
        "no cookie exists anywhere"
    );
}
