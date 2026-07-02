//! The i18n-catalog routes over the real gateway (i18n-catalogs scope, the browser path). Mirrors the
//! host MCP verbs 1:1 at the transport boundary: `message.render` for self + fan-out, `prefs.catalog`
//! merged fetch, `message.set_catalog` admin write, capability-deny per gated verb, and two-session
//! workspace isolation. The gateway re-checks every cap server-side; the workspace + principal come
//! from the token (§7).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt;

const RENDER: &str = "mcp:message.render:call";
const RENDER_RECIP: &str = "mcp:message.render_recipient:call";
const CATALOG: &str = "mcp:prefs.catalog:call";
const SET_CATALOG: &str = "mcp:message.set_catalog:call";
const SET_PREFS: &str = "mcp:prefs.set:call";

fn put_req(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_catalog_then_render_uses_override() {
    let (gw, key) = gateway().await;
    let admin = token(&key, "user:adm", "acme", &[SET_CATALOG, RENDER, CATALOG]);

    // PUT /message/catalog (admin) -> 204
    let resp = router(gw.clone())
        .oneshot(bearer(
            put_req(
                "/message/catalog",
                json!({ "locale": "en", "messages": { "notify.welcome": "Hey {name}!" } }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // POST /message/render uses the override for the caller's resolved language (en builtin fallback).
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/message/render",
                json!({ "key": "notify.welcome", "args": { "name": "Ada" } }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert_eq!(body["text"], "Hey Ada!");
    assert_eq!(body["locale_used"], "en");

    // POST /prefs/catalog returns the merged map with the override.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/prefs/catalog", json!({ "locale": "en" })),
            &admin,
        ))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    assert_eq!(body["messages"]["notify.welcome"], "Hey {name}!");
    assert_eq!(body["has_override"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn render_for_another_recipient_needs_fanout_grant() {
    let (gw, key) = gateway().await;
    // Only the base render grant -> rendering for another recipient is a 403.
    let tok = token(&key, "user:ada", "acme", &[RENDER]);
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/message/render",
                json!({ "key": "notify.welcome", "args": { "name": "Ada" }, "recipient": "user:bob" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_catalog_denied_without_admin_cap() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:bob", "acme", &[RENDER, CATALOG]); // no SET_CATALOG
    let resp = router(gw.clone())
        .oneshot(bearer(
            put_req(
                "/message/catalog",
                json!({ "locale": "es", "messages": { "notify.welcome": "Hola" } }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn render_denied_without_any_grant() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:eve", "acme", &[]);
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/message/render",
                json!({ "key": "notify.welcome", "args": {} }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn out_of_subset_override_is_bad_request_not_denied() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:adm", "acme", &[SET_CATALOG]);
    let resp = router(gw.clone())
        .oneshot(bearer(
            put_req(
                "/message/catalog",
                json!({ "locale": "en", "messages": { "x": "{n, spellout}" } }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    // A lint failure is the caller's fault (400), distinct from a capability denial (403).
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_sessions_render_per_workspace() {
    // The SAME override key differs per workspace over the gateway; a render in ws-B never reads ws-A.
    let (gw, key) = gateway().await;
    let tok_a = token(&key, "user:adm", "ws-a", &[SET_CATALOG, RENDER]);
    let tok_b = token(&key, "user:adm", "ws-b", &[SET_CATALOG, RENDER]);

    router(gw.clone())
        .oneshot(bearer(
            put_req(
                "/message/catalog",
                json!({ "locale": "en", "messages": { "notify.welcome": "A: {name}" } }),
            ),
            &tok_a,
        ))
        .await
        .unwrap();
    router(gw.clone())
        .oneshot(bearer(
            put_req(
                "/message/catalog",
                json!({ "locale": "en", "messages": { "notify.welcome": "B: {name}" } }),
            ),
            &tok_b,
        ))
        .await
        .unwrap();

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/message/render",
                json!({ "key": "notify.welcome", "args": { "name": "Z" } }),
            ),
            &tok_b,
        ))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    assert_eq!(body["text"], "B: Z"); // ws-b's override, never ws-a's
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fanout_two_members_two_renders_over_gateway() {
    let (gw, key) = gateway().await;
    let ws = "team";
    // Seed two members' prefs via the real gateway PUT /prefs (real write path, no seed shortcut).
    let ada = token(&key, "user:ada", ws, &[SET_PREFS]);
    let bob = token(&key, "user:bob", ws, &[SET_PREFS]);
    router(gw.clone())
        .oneshot(bearer(put_req("/prefs", json!({ "language": "es", "timezone": "Europe/Madrid", "date_style": "eu", "number_format": "comma_dot" })), &ada))
        .await
        .unwrap();
    router(gw.clone())
        .oneshot(bearer(put_req("/prefs", json!({ "language": "en", "timezone": "America/New_York", "date_style": "usa", "unit_overrides": { "speed": "knot" } })), &bob))
        .await
        .unwrap();

    let producer = token(&key, "svc:outbox", ws, &[RENDER, RENDER_RECIP]);
    let ts_ms = 1_751_373_000_000i64;
    let render_for = |who: &str| json!({ "key": "alert.threshold_crossed", "args": { "name": "S", "limit": 12.0, "ts": ts_ms }, "recipient": who });

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/message/render", render_for("user:ada")),
            &producer,
        ))
        .await
        .unwrap();
    let a: Value = json_body(resp).await;
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/message/render", render_for("user:bob")),
            &producer,
        ))
        .await
        .unwrap();
    let b: Value = json_body(resp).await;

    assert_eq!(a["locale_used"], "es");
    assert_eq!(b["locale_used"], "en");
    assert!(
        a["text"].as_str().unwrap().contains("43,2 km/h"),
        "es: {}",
        a["text"]
    );
    assert!(
        b["text"].as_str().unwrap().contains("23.3 kn"),
        "en: {}",
        b["text"]
    );
    assert_ne!(a["text"], b["text"]);
}
