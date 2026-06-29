//! The prefs + formatting routes over the real gateway (prefs scope, the browser path). Mirrors the
//! host MCP verbs 1:1 at the transport boundary: the `prefs.*` gated CRUD round-trip, the grant-free
//! `format.*`/`convert.*` utility tier (usable with NO prefs cap), capability-deny per gated verb,
//! and two-session workspace isolation. The gateway re-checks every cap server-side; the workspace +
//! principal come from the token (§7).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

const SET: &str = "mcp:prefs.set:call";
const GET: &str = "mcp:prefs.get:call";
const RESOLVE: &str = "mcp:prefs.resolve:call";

fn put_req(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefs_set_get_resolve_round_trip() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &[SET, GET, RESOLVE]);

    // PUT /prefs (set) -> 204
    let resp = router(gw.clone())
        .oneshot(bearer(
            put_req(
                "/prefs",
                json!({ "language": "es", "unit_system": "imperial" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // GET /prefs returns the stored nullable record.
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/prefs"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert_eq!(body["prefs"]["language"], "es");
    assert_eq!(body["prefs"]["unit_system"], "imperial");

    // POST /prefs/resolve folds the chain; an unset axis falls back to built-in.
    let resp = router(gw.clone())
        .oneshot(bearer(json_post("/prefs/resolve", json!({})), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert_eq!(body["resolved"]["language"], "es");
    assert_eq!(body["resolved"]["timezone"], "UTC");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn format_and_convert_need_no_prefs_cap() {
    let (gw, key) = gateway().await;
    // A session with NO prefs caps at all still reaches the utility tier.
    let tok = token(&key, "user:eve", "acme", &[]);

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/convert/unit",
                json!({ "value": 100.0, "from": "celsius", "to": "fahrenheit" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert!((body["value"].as_f64().unwrap() - 212.0).abs() < 1e-6);

    // format.quantity with an inline resolved prefs object.
    let prefs = json!({
        "language": "en", "timezone": "UTC", "date_style": "iso", "time_style": "h24",
        "first_day_of_week": "monday", "number_format": "dot_comma", "unit_system": "metric",
        "unit_overrides": { "speed": "knot" }
    });
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/format/quantity",
                json!({ "value": 12.0, "from_unit": "meter_per_second", "dimension": "speed", "prefs": prefs }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert_eq!(body["text"], "23.3 kn");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_default_denied_without_admin_cap() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:bob", "acme", &[SET]); // no SET_DEFAULT
    let resp = router(gw.clone())
        .oneshot(bearer(
            put_req("/prefs/default", json!({ "unit_system": "imperial" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefs_get_denied_without_cap() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:eve", "acme", &[]);
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/prefs"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_sessions_resolve_per_workspace() {
    // The SAME global user in two workspaces resolves to each workspace's own value over the gateway.
    let (gw_a, key) = gateway().await;
    let tok_a = token(&key, "user:ada", "ws-a", &[SET, RESOLVE]);
    let tok_b = token(&key, "user:ada", "ws-b", &[SET, RESOLVE]);

    router(gw_a.clone())
        .oneshot(bearer(
            put_req("/prefs", json!({ "timezone": "Asia/Tokyo" })),
            &tok_a,
        ))
        .await
        .unwrap();
    router(gw_a.clone())
        .oneshot(bearer(
            put_req("/prefs", json!({ "timezone": "Europe/Madrid" })),
            &tok_b,
        ))
        .await
        .unwrap();

    let resp = router(gw_a.clone())
        .oneshot(bearer(json_post("/prefs/resolve", json!({})), &tok_b))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    assert_eq!(body["resolved"]["timezone"], "Europe/Madrid"); // ws-b's value, never ws-a's
}
