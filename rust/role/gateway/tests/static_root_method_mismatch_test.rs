//! The static-root **method-mismatch** rule (spa-static-hosting scope) — the gap that shipped the
//! bug. `static_root_test.rs` covers the *no route matched* fallback; NONE of its 5 tests covered a
//! path that matched a route but not for its method, which is why `GET /login` 405'd on a deployed
//! shell (lb registers `POST /login`) and ems's ARM/Pi build could serve its whole UI but never
//! render a login page (NubeIO/ems#8).
//!
//! The rule under test: method-mismatch + GET/HEAD + `Accept` **explicitly** prefers `text/html`
//! → `index.html`; anything else → the 405 with `Allow` intact.
//!
//! Two of these are guards against fixing the bug *wrongly*, and matter more than the happy path:
//!   - `method_mismatch_405_keeps_its_allow_header` — a naive fallback silently drops `Allow`.
//!   - `curl_default_accept_does_not_get_html` — `*/*` must NOT be html-preferring, or
//!     `curl -X GET /mcp/call` starts returning a web page instead of an API error.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::{router, Gateway};
use tower::ServiceExt;

/// A gateway whose static root is a temp fixture holding an `index.html`.
async fn gateway_with_static_root(tag: &str) -> (Gateway, std::path::PathBuf) {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let dir = std::env::temp_dir().join(format!("lb-spa-mismatch-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("index.html"),
        b"<!doctype html><title>shell</title>\n",
    )
    .unwrap();
    let gw = Gateway::new(node, key, NOW).with_static_root(dir.clone());
    (gw, dir)
}

/// A request with an explicit `Accept`.
fn req_accept(method: &str, uri: &str, accept: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("accept", accept)
        .body(Body::empty())
        .unwrap()
}

/// The browser navigation header Chrome/Firefox actually send.
const BROWSER_ACCEPT: &str =
    "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8";

/// **The regression test for NubeIO/ems#8.** A browser navigating to `/login` — a path where lb has
/// only a POST handler — must reach the SPA shell, not a 405.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn browser_navigation_to_login_reaches_the_spa() {
    let (gw, dir) = gateway_with_static_root("login-nav").await;
    let resp = router(gw)
        .oneshot(req_accept("GET", "/login", BROWSER_ACCEPT))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "GET /login serves the shell");
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(
        body.starts_with(b"<!doctype html>"),
        "GET /login → index.html so the SPA can render its login page"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// An API client asking the same path keeps today's API contract exactly.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn api_client_on_a_method_mismatch_still_gets_405() {
    let (gw, dir) = gateway_with_static_root("json-405").await;
    let resp = router(gw)
        .oneshot(req_accept("GET", "/login", "application/json"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    let _ = std::fs::remove_dir_all(dir);
}

/// **Trap 1.** axum does not pass the allowed-method set into the fallback, so a hand-rolled 405 can
/// silently ship without `Allow` — a regression against today's behaviour that ems's own diagnosis
/// relied on. (It is preserved because axum re-attaches it on the way out, but that is exactly the
/// kind of thing a refactor breaks silently, so it is pinned here.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn method_mismatch_405_keeps_its_allow_header() {
    let (gw, dir) = gateway_with_static_root("allow-hdr").await;
    let resp = router(gw)
        .oneshot(req_accept("GET", "/login", "application/json"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        resp.headers().get("allow").map(|v| v.to_str().unwrap()),
        Some("POST"),
        "the 405 must still tell the client which method to use"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// **Trap 2 — the most likely implementation bug.** `*/*` is curl's default and does NOT mean "I want
/// a web page". If this regresses, every `curl -X GET` against a POST-only API route silently starts
/// returning HTML instead of a 405.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn curl_default_accept_does_not_get_html() {
    let (gw, dir) = gateway_with_static_root("curl-star").await;
    let resp = router(gw)
        .oneshot(req_accept("GET", "/mcp/call", "*/*"))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "Accept: */* is not an html preference — the API contract holds"
    );
    assert_eq!(
        resp.headers().get("allow").map(|v| v.to_str().unwrap()),
        Some("POST")
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// A request with NO `Accept` header at all (the Rust client, most SDKs) keeps the 405.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn no_accept_header_does_not_get_html() {
    let (gw, dir) = gateway_with_static_root("no-accept").await;
    let resp = router(gw).oneshot(get_req("/mcp/call")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    let _ = std::fs::remove_dir_all(dir);
}

/// The deliberate, documented cost of content negotiation: a *browser* hand-navigating to a POST-only
/// API route gets the shell. Pinned so the next reader sees it was chosen, not missed (scope → Risks:
/// "worth stating so the next reader doesn't 'fix' it"). API clients never send this header.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn browser_navigating_to_an_api_route_gets_the_shell_by_design() {
    let (gw, dir) = gateway_with_static_root("api-nav").await;
    let resp = router(gw)
        .oneshot(req_accept("GET", "/mcp/call", BROWSER_ACCEPT))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let _ = std::fs::remove_dir_all(dir);
}

/// HEAD is a navigation too — and axum strips the body for a top-level HEAD itself.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn head_navigation_serves_index_with_no_body() {
    let (gw, dir) = gateway_with_static_root("head-nav").await;
    let resp = router(gw)
        .oneshot(req_accept("HEAD", "/login", BROWSER_ACCEPT))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(body.is_empty(), "HEAD carries no body");
    let _ = std::fs::remove_dir_all(dir);
}

/// A non-navigation method mismatch is never a page, however the client asks.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_non_get_method_mismatch_is_never_html() {
    let (gw, dir) = gateway_with_static_root("delete-405").await;
    let req = Request::builder()
        .method("DELETE")
        .uri("/login")
        .header("accept", BROWSER_ACCEPT)
        .body(Body::empty())
        .unwrap();
    let resp = router(gw).oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "only GET/HEAD navigate; a DELETE is an API call whatever it Accepts"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// The real route is untouched: `POST /login` still reaches the login handler and mints a real token.
/// (This is lb's legacy dev-login — it trusts the caller and issues a signed token; the credential
/// check is a separate seam. The point here is that the handler answers, not the fallback: the reply
/// is a token, never a 405 and never the static page.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_real_post_route_is_unaffected() {
    let (gw, dir) = gateway_with_static_root("post-login").await;
    let body = serde_json::json!({ "user": "nobody", "workspace": "ws1" });
    let resp = router(gw).oneshot(json_post("/login", body)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(
        !body.starts_with(b"<!doctype html>"),
        "POST /login is the login handler's JSON, never the static shell"
    );
    let v: serde_json::Value = serde_json::from_slice(&body).expect("login returns JSON");
    assert!(
        v.get("token")
            .and_then(|t| t.as_str())
            .is_some_and(|t| !t.is_empty()),
        "the real login still mints a token"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// **`static_root: None` ⇒ routing is byte-for-byte today's.** The fallback is mounted only alongside
/// a static root, so a node with no shell (rubixd, rubix-ai, every existing deploy) sees no change:
/// the 405 is axum's own, `Allow` intact, and no static page exists anywhere.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn without_a_static_root_a_browser_still_gets_the_405() {
    let (gw, _key) = gateway().await;
    let resp = router(gw)
        .oneshot(req_accept("GET", "/login", BROWSER_ACCEPT))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "no shell to serve ⇒ today's routing exactly"
    );
    assert_eq!(
        resp.headers().get("allow").map(|v| v.to_str().unwrap()),
        Some("POST")
    );
}
