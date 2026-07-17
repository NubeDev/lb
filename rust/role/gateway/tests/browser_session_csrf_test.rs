//! **The gate on the browser-session scope shipping at all** (browser-session scope → Risks): CSRF,
//! plus the two mandatory categories (capability-deny, workspace-isolation) driven THROUGH `/api/*`.
//!
//! Why this is a separate file from `browser_session_test.rs`: those are the seam's behaviour; these
//! are the properties that make it safe to exist. The scope is explicit that the cross-origin test
//! "is the gate on this scope shipping at all", and that the seam "must be provably incapable of
//! widening" authority. Real gateway, real store, real argon2, real caps — no mocks (CLAUDE §9).

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

const HOST: &str = "pi.local:8391";

async fn session_gateway_on(node: Arc<Node>, key: &SigningKey) -> Gateway {
    Gateway::new(node, key.clone(), NOW)
        .with_global_credential_check(Arc::new(GlobalPasswordHash))
        .with_browser_session(BrowserSessionConfig::default())
}

async fn session_gateway() -> (Gateway, Arc<Node>, SigningKey) {
    let node = Arc::new(Node::boot_as(lb_host::Role::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = session_gateway_on(node.clone(), &key).await;
    (gw, node, key)
}

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

async fn seed_person(gw: &Gateway, admin: &str, sub: &str, email: &str, password: &str) {
    for (uri, body) in [
        ("/admin/identities", json!({ "sub": sub, "email": email })),
        (
            "/admin/identities/PLACEHOLDER/password",
            json!({ "secret": password }),
        ),
        ("/admin/members", json!({ "sub": sub })),
    ] {
        let uri = uri.replace("PLACEHOLDER", sub);
        let st = router(gw.clone())
            .oneshot(bearer(json_post(&uri, body), admin))
            .await
            .unwrap()
            .status();
        assert!(st.is_success(), "seed step {uri}: {st}");
    }
}

/// A same-origin browser POST.
fn same_origin_post(uri: &str, body: Value) -> Request<Body> {
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

fn with_cookie(req: Request<Body>, sid: &str) -> Request<Body> {
    let (mut parts, body) = req.into_parts();
    parts
        .headers
        .insert("cookie", format!("lb_session={sid}").parse().unwrap());
    Request::from_parts(parts, body)
}

fn sid_from(resp: &axum::response::Response) -> String {
    resp.headers()
        .get("set-cookie")
        .expect("a session cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .strip_prefix("lb_session=")
        .unwrap()
        .to_string()
}

async fn login(gw: &Gateway, email: &str, password: &str) -> String {
    let resp = router(gw.clone())
        .oneshot(same_origin_post(
            "/api/auth/login",
            json!({ "email": email, "password": password }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "login {email}");
    sid_from(&resp)
}

/// **THE test. A cross-origin POST carrying a perfectly valid cookie must be rejected.**
///
/// This is the attack cookie auth invites: evil.com submits a form/fetch to the Pi, the browser
/// helpfully attaches the victim's session cookie, and without this gate the write lands. `SameSite=Lax`
/// is the first line; this is the second, because `Lax` is a same-*site* check and some clients/browsers
/// don't enforce it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_cross_origin_post_with_a_valid_cookie_is_rejected() {
    let (gw, _node, _key) = session_gateway().await;
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(&gw, &admin, "user:ada", "ada@example.com", "hunter2hunter2").await;
    let sid = login(&gw, "ada@example.com", "hunter2hunter2").await;

    // Same request, same valid cookie — but the browser says it came from another site.
    let evil = Request::builder()
        .method("POST")
        .uri("/api/mcp/call")
        .header("content-type", "application/json")
        .header("host", HOST)
        .header("origin", "http://evil.example")
        .header("sec-fetch-site", "cross-site")
        .body(Body::from(
            serde_json::to_vec(&json!({ "tool": "series.find", "args": {} })).unwrap(),
        ))
        .unwrap();
    let resp = router(gw.clone())
        .oneshot(with_cookie(evil, &sid))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "a cross-origin write with a valid cookie MUST be rejected"
    );
}

/// The same call, same-origin, succeeds — proving the gate rejects the attacker, not the shell.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_same_call_same_origin_succeeds() {
    let (gw, _node, _key) = session_gateway().await;
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(&gw, &admin, "user:ada", "ada@example.com", "hunter2hunter2").await;
    let sid = login(&gw, "ada@example.com", "hunter2hunter2").await;

    let resp = router(gw.clone())
        .oneshot(with_cookie(
            same_origin_post(
                "/api/mcp/call",
                json!({ "tool": "series.find", "args": { "tags": [] } }),
            ),
            &sid,
        ))
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "the real shell is not blocked"
    );
    assert_ne!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// A cross-origin POST whose `Origin` lies about the host is rejected on the `Origin` fallback path
/// (no `Sec-Fetch-Site` — an older browser or a proxy that strips it).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_foreign_origin_without_sec_fetch_site_is_rejected() {
    let (gw, _node, _key) = session_gateway().await;
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(&gw, &admin, "user:ada", "ada@example.com", "hunter2hunter2").await;
    let sid = login(&gw, "ada@example.com", "hunter2hunter2").await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/mcp/call")
        .header("content-type", "application/json")
        .header("host", HOST)
        .header("origin", "http://evil.example")
        .body(Body::from(
            serde_json::to_vec(&json!({ "tool": "series.find", "args": {} })).unwrap(),
        ))
        .unwrap();
    let resp = router(gw.clone())
        .oneshot(with_cookie(req, &sid))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

/// A POST with NO origin evidence at all is rejected: "no evidence of same-origin" is not a pass.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_post_with_no_origin_evidence_is_rejected() {
    let (gw, _node, _key) = session_gateway().await;
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(&gw, &admin, "user:ada", "ada@example.com", "hunter2hunter2").await;
    let sid = login(&gw, "ada@example.com", "hunter2hunter2").await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/mcp/call")
        .header("content-type", "application/json")
        .header("host", HOST)
        .body(Body::from(
            serde_json::to_vec(&json!({ "tool": "series.find", "args": {} })).unwrap(),
        ))
        .unwrap();
    let resp = router(gw.clone())
        .oneshot(with_cookie(req, &sid))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

/// **Mandatory: capability-deny through `/api/*`.** A session whose caps lack the verb gets the same
/// `403` as the bearer path. The seam attaches a bearer; it never widens one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn capability_deny_holds_through_the_seam() {
    let (gw, _node, _key) = session_gateway().await;
    let admin = bootstrap_admin(&gw, "user:root", "acme").await;
    // A plain member: no admin caps.
    seed_person(
        &gw,
        &admin,
        "user:mallory",
        "mallory@example.com",
        "hunter2hunter2",
    )
    .await;
    let sid = login(&gw, "mallory@example.com", "hunter2hunter2").await;

    // An admin-only verb, through the seam.
    let resp = router(gw.clone())
        .oneshot(with_cookie(
            same_origin_post("/api/admin/members", json!({ "sub": "user:eve" })),
            &sid,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "a member's session cannot reach an admin verb through /api/*"
    );
}

/// **Mandatory: workspace-isolation through `/api/*`.** A session in ws A cannot read ws B's rows.
/// Both workspaces live on ONE node (the real isolation setup), so this proves the wall, not a
/// missing database.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_holds_through_the_seam() {
    let (gw, node, key) = session_gateway().await;

    // Two workspaces on one node. `user:ada` belongs to `acme` only.
    let acme_admin = bootstrap_admin(&gw, "user:root", "acme").await;
    seed_person(
        &gw,
        &acme_admin,
        "user:ada",
        "ada@example.com",
        "hunter2hunter2",
    )
    .await;

    // A separate workspace with its own admin + a channel only IT should see.
    let other = session_gateway_on(node, &key).await;
    let other_admin = bootstrap_admin(&other, "user:boss", "globex").await;
    let created = router(other.clone())
        .oneshot(bearer(
            json_post("/channels", json!({ "channel": "globex-secret" })),
            &other_admin,
        ))
        .await
        .unwrap();
    assert!(
        created.status().is_success(),
        "seed globex's channel: {}",
        created.status()
    );

    // Ada's session (workspace `acme`) asks for channels through the seam.
    let sid = login(&gw, "ada@example.com", "hunter2hunter2").await;
    let resp = router(gw.clone())
        .oneshot(with_cookie(
            Request::builder()
                .method("GET")
                .uri("/api/channels")
                .header("host", HOST)
                .body(Body::empty())
                .unwrap(),
            &sid,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(
        !text.contains("globex-secret"),
        "ISO LEAK: a session in `acme` saw `globex`'s rows through /api/*: {text}"
    );
}
