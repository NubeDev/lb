//! The pre-auth brand read over the REAL gateway (workspace-branding scope, the public read seam):
//! `GET /public/branding?ws=<ws>` with **no bearer at all**.
//!
//! This route is a deliberate break in the workspace wall, so the tests are written as the wall's
//! guard rather than as a happy-path round-trip. They assert, against a real node and a real store:
//! the brand an admin set is served without a token; **workspace isolation** (A's brand for A, never
//! B's); the **field whitelist** (the body carries the two brand axes and nothing else, proven by a
//! record whose every OTHER axis is also set); and that unknown / unbranded / malformed / missing
//! `ws` are byte-for-byte identical, so the route is not a workspace-existence oracle. The per-IP
//! ceiling is exercised last, on its own client key (the limiter is process-wide).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use lb_role_gateway::{router, PUBLIC_BRANDING_MAX_PER_WINDOW};
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

const SET_DEFAULT: &str = "mcp:prefs.set_default:call";

/// The unauthenticated request the sign-in screen makes. `ip` keys the per-IP limiter, so every test
/// uses its own and none can spend another's budget.
fn brand_req(uri: &str, ip: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .unwrap()
}

fn put_req(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

/// Set `ws`'s workspace-default prefs as an admin — the ONLY way this data gets written. There is no
/// public write half; the pre-auth route is read-only by construction.
async fn set_default(
    gw: &lb_role_gateway::Gateway,
    key: &lb_auth::SigningKey,
    ws: &str,
    body: Value,
) {
    let admin = token(key, "user:admin", ws, &[SET_DEFAULT]);
    let resp = router(gw.clone())
        .oneshot(bearer(put_req("/prefs/default", body), &admin))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "admin sets {ws}'s default"
    );
}

/// The core of the fix: a browser that has NEVER signed in gets the deployment's brand, not the
/// compiled product default. No `Authorization` header is sent anywhere in this test.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn serves_the_workspace_brand_with_no_token() {
    let (gw, key) = gateway().await;
    set_default(
        &gw,
        &key,
        "nube",
        json!({
            "ui_branding": { "siteName": "ESR", "tagline": "building intelligence" },
            "ui_theme": { "preset": "corporate", "mode": "dark" }
        }),
    )
    .await;

    let resp = router(gw.clone())
        .oneshot(brand_req("/public/branding?ws=nube", "203.0.113.10"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok()),
        Some("public, max-age=60"),
        "login is a hot path — the brand is briefly cacheable, but not pinned"
    );
    let body: Value = json_body(resp).await;
    assert_eq!(body["ui_branding"]["siteName"], "ESR");
    assert_eq!(body["ui_branding"]["tagline"], "building intelligence");
    assert_eq!(body["ui_theme"]["preset"], "corporate");
    assert_eq!(body["ui_theme"]["mode"], "dark");
}

/// **Workspace isolation (mandatory).** Two real workspaces on one node, each with its own brand: the
/// public route serves each workspace its own and never the other's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn serves_only_the_named_workspaces_brand() {
    let (gw, key) = gateway().await;
    set_default(
        &gw,
        &key,
        "alpha",
        json!({ "ui_branding": { "siteName": "Alpha Co" } }),
    )
    .await;
    set_default(
        &gw,
        &key,
        "beta",
        json!({ "ui_branding": { "siteName": "Beta Co" } }),
    )
    .await;

    let a = router(gw.clone())
        .oneshot(brand_req("/public/branding?ws=alpha", "203.0.113.11"))
        .await
        .unwrap();
    let a: Value = json_body(a).await;
    let b = router(gw.clone())
        .oneshot(brand_req("/public/branding?ws=beta", "203.0.113.11"))
        .await
        .unwrap();
    let b: Value = json_body(b).await;

    assert_eq!(a["ui_branding"]["siteName"], "Alpha Co");
    assert_eq!(b["ui_branding"]["siteName"], "Beta Co");
    assert!(
        !serde_json::to_string(&a).unwrap().contains("Beta"),
        "workspace A's response must not carry a byte of workspace B"
    );
}

/// **The wall break, guarded.** The workspace-default record carries every prefs axis — i18n, units,
/// the lot — and this route serves TWO of them. The response object's key set is asserted exactly,
/// and the raw body is searched for each non-brand value that IS in the record, so a future axis
/// added to `Prefs` cannot reach the public internet by simply existing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn body_carries_the_brand_axes_and_nothing_else() {
    let (gw, key) = gateway().await;
    set_default(
        &gw,
        &key,
        "nube",
        json!({
            "language": "es",
            "timezone": "Australia/Brisbane",
            "date_style": "usa",
            "time_style": "h12",
            "number_format": "comma_dot",
            "unit_system": "imperial",
            "ui_branding": { "siteName": "ESR" },
            "ui_theme": { "preset": "corporate" }
        }),
    )
    .await;

    let resp = router(gw.clone())
        .oneshot(brand_req("/public/branding?ws=nube", "203.0.113.12"))
        .await
        .unwrap();
    let raw = body_text(resp).await;
    let body: Value = serde_json::from_str(&raw).unwrap();

    let mut keys: Vec<&str> = body
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["ui_branding", "ui_theme"],
        "the public body is a two-field whitelist, not a prefs record"
    );
    // Every non-brand axis set above, by NAME and by VALUE. (By value alone — `language: "es"` —
    // would be a useless probe: a two-letter code appears inside unrelated words like `preset`. The
    // axis names are the reliable probe, the distinctive values the corroboration.)
    for leaked in [
        "language",
        "timezone",
        "date_style",
        "time_style",
        "number_format",
        "unit_system",
        "Australia/Brisbane",
        "usa",
        "h12",
        "comma_dot",
        "imperial",
    ] {
        assert!(
            !raw.contains(leaked),
            "non-brand axis {leaked:?} leaked through the pre-auth route: {raw}"
        );
    }
}

/// **Not a workspace-existence oracle.** An unbranded workspace, a workspace that does not exist, a
/// slug the store itself rejects, and an omitted `ws` all answer identically — same status, same
/// bytes. A caller learns nothing it did not already assert.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_miss_answers_identically() {
    let (gw, key) = gateway().await;
    // A REAL workspace that exists and has prefs, but no brand — its answer must equal the answers
    // for workspaces that do not exist at all.
    set_default(&gw, &key, "nube", json!({ "language": "es" })).await;

    let mut bodies = Vec::new();
    for uri in [
        "/public/branding?ws=nube",           // exists, has prefs, unbranded
        "/public/branding?ws=ghost",          // never provisioned
        "/public/branding?ws=%20",            // blank after trim
        "/public/branding?ws=bad%20ws%60%3B", // rejected by the store's slug guard (space, backtick, ;)
        "/public/branding",                   // no ws at all
    ] {
        let resp = router(gw.clone())
            .oneshot(brand_req(uri, "203.0.113.13"))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "{uri} must not signal by status"
        );
        bodies.push((uri, body_text(resp).await));
    }
    let (_, first) = &bodies[0];
    assert_eq!(first, r#"{"ui_branding":null,"ui_theme":null}"#);
    for (uri, body) in &bodies {
        assert_eq!(
            body, first,
            "{uri} answered differently — that is an oracle"
        );
    }
}

/// The route ships rate-limited, per client key, like the other `/public/*` routes. Its own IP, so
/// the burst cannot starve the tests above (the limiter is process-wide).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn is_rate_limited_per_client() {
    let (gw, _key) = gateway().await;
    let app = router(gw);
    let ip = "203.0.113.14";

    // The window is wall-clock; if it rolls mid-burst the extra hits land in a fresh window, so allow
    // one retry round (a broken limiter still fails both).
    let mut limited = false;
    for _round in 0..2 {
        for i in 0..=PUBLIC_BRANDING_MAX_PER_WINDOW {
            let res = app
                .clone()
                .oneshot(brand_req("/public/branding?ws=nube", ip))
                .await
                .unwrap();
            if res.status() == StatusCode::TOO_MANY_REQUESTS {
                limited = true;
                break;
            }
            assert_eq!(
                res.status(),
                StatusCode::OK,
                "hit {i} must reach the handler"
            );
        }
        if limited {
            break;
        }
    }
    assert!(limited, "the (MAX+1)-th read from one client must be 429");

    // A different client is untouched — the ceiling is per key, never global.
    let res = app
        .clone()
        .oneshot(brand_req("/public/branding?ws=nube", "203.0.113.15"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
