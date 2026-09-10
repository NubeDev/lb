//! The **contractor's public routes**, over the REAL router (case-plane scope §"Gateway — the token
//! principal", wave 2).
//!
//! Everything here is real: a booted `Node`, the real `router()`, the real caps wall, the real
//! outbox relay. The one permitted external is the email provider — and it is what makes these
//! tests honest, because the token they present is read out of the mail that was actually sent. The
//! store keeps only a hash, so there is no other way to get one, which is exactly the property
//! under test.
//!
//! What it pins:
//!   - `GET /r/{token}` is a PAGE, not a data route — it must not answer JSON;
//!   - the three `/public/case/request*` routes work with no session at all;
//!   - a withdrawn or expired token is **410** with a body that never says which;
//!   - an unknown token is **404**, and never a login prompt;
//!   - the routes are per-IP rate limited (the same fixed-window limiter the invite route uses).

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use lb_host::{call_tool, relay_outbox, EmailTarget, Node, RecordingEmailProvider};
use lb_role_gateway::{router, INVITE_ACCEPT_MAX_PER_WINDOW};
use serde_json::{json, Value};
use tower::ServiceExt;

const WS: &str = "nube";
/// The gateway's clock is SECONDS; the case plane's is millis. `common::NOW` is 1000 s.
const NOW_MS: u64 = common::NOW * 1000;

/// Boot a gateway, provision an admin, and raise one ask — returning the router, the request id and
/// the raw token as it appeared in the mail.
async fn rig() -> (axum::Router, String, String) {
    let (gw, key) = common::gateway().await;
    let session = common::bootstrap::provision_admin(&gw, "user:test", WS).await;
    let node = gw.node.clone();

    // Seed through the real verbs, under the REAL provisioned admin session — the caps are whatever
    // the durable grants resolve to, not a hand-written list.
    let principal = lb_auth::verify(&key, &session, common::NOW).expect("the admin session");
    let raised = tool(
        &node,
        &principal,
        "insight.raise",
        json!({
            "dedup_key": "gw-probe",
            "severity": "warning",
            "title": "supply temp above setpoint",
            "origin": { "kind": "rule", "ref": "rule:probe" },
            "ts": NOW_MS,
        }),
    )
    .await;
    let insight = raised["id"].as_str().unwrap().to_string();
    let case_id = tool(&node, &principal, "insight.get", json!({ "id": insight })).await["case_id"]
        .as_str()
        .expect("the grouping reactor opened a case")
        .to_string();

    tool(
        &node,
        &principal,
        "party.upsert",
        json!({
            "id": "northern",
            "kind": "contractor",
            "name": "Northern Mechanical",
            "contact": { "email": "ops@northern.example" },
            "default_ask_window_h": 24,
        }),
    )
    .await;
    let request = tool(
        &node,
        &principal,
        "case.request.send",
        json!({
            "case_id": case_id,
            "party_id": "northern",
            "ask": "quote",
            "brief": { "ask_text": "Please price the actuator.", "currency": "AUD" },
            "ts": NOW_MS,
        }),
    )
    .await;
    let request_id = request["id"].as_str().unwrap().to_string();

    // The link email, through the real relay — the only place the raw token exists.
    let provider = Arc::new(RecordingEmailProvider::default());
    let target = EmailTarget::new(Box::new(provider.clone()), node.store.clone());
    relay_outbox(&node.store, WS, &target, common::NOW)
        .await
        .expect("a relay pass");
    let sends = provider.sends();
    assert_eq!(sends.len(), 1, "the ask must have mailed a link");
    let raw = token_from(&sends[0].body);

    (router(gw), request_id, raw)
}

async fn tool(node: &Arc<Node>, p: &lb_auth::Principal, name: &str, input: Value) -> Value {
    let out = call_tool(node, p, WS, name, &input.to_string())
        .await
        .unwrap_or_else(|e| panic!("{name} failed: {e:?}"));
    serde_json::from_str(&out).unwrap_or(Value::Null)
}

fn token_from(body: &str) -> String {
    let at = body
        .find("/r/")
        .unwrap_or_else(|| panic!("no link in the mail: {body}"));
    body[at + 3..]
        .split_whitespace()
        .next()
        .expect("a token")
        .trim_end_matches(['.', ',', '<', '"'])
        .to_string()
}

async fn body_of(res: axum::response::Response) -> String {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8_lossy(&bytes).to_string()
}

fn get(uri: &str, ip: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .unwrap()
}

fn post_json(uri: &str, ip: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// The whole round trip, with no session anywhere: view → reply → the replied state.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_token_views_and_replies_with_no_session_at_all() {
    let (app, _id, token) = rig().await;

    let res = app
        .clone()
        .oneshot(get(
            &format!("/public/case/request?token={token}"),
            "203.0.113.20",
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let view: Value = serde_json::from_str(&body_of(res).await).expect("json");
    assert_eq!(view["party_name"].as_str(), Some("Northern Mechanical"));
    assert_eq!(view["ask"].as_str(), Some("quote"));
    assert_eq!(view["title"].as_str(), Some("supply temp above setpoint"));
    assert_eq!(
        view["ask_text"].as_str(),
        Some("Please price the actuator.")
    );
    assert_eq!(view["currency"].as_str(), Some("AUD"));
    assert!(
        view.get("case_id").is_none(),
        "the page must not learn the case id: {view}"
    );
    assert!(view.get("reply").is_none(), "nothing answered yet: {view}");

    // The reply carries the token in the BODY, so it never lands in an access log.
    let res = app
        .clone()
        .oneshot(post_json(
            "/public/case/request/reply",
            "203.0.113.20",
            json!({
                "token": token,
                "reply": { "kind": "quote", "amount": 1180.0, "currency": "AUD", "text": "parts + labour" },
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let receipt: Value = serde_json::from_str(&body_of(res).await).expect("json");
    assert_eq!(receipt["recorded"].as_bool(), Some(true));

    // The page can now render what they already told us.
    let res = app
        .clone()
        .oneshot(get(
            &format!("/public/case/request?token={token}"),
            "203.0.113.20",
        ))
        .await
        .unwrap();
    let view: Value = serde_json::from_str(&body_of(res).await).expect("json");
    assert_eq!(view["reply"]["kind"].as_str(), Some("quote"));
    assert_eq!(view["reply"]["amount"].as_f64(), Some(1180.0));
}

/// `GET /r/{token}` is a PAGE url. It must not be a data route — the browser has to be served the
/// app there, and one path cannot answer HTML to a browser and JSON to that page's fetch.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_page_url_is_not_a_data_route() {
    let (app, _id, token) = rig().await;
    let res = app
        .oneshot(get(&format!("/r/{token}"), "203.0.113.21"))
        .await
        .unwrap();
    let status = res.status();
    let body = body_of(res).await;
    assert!(
        serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|v| v.get("request_id").cloned())
            .is_none(),
        "/r/{{token}} must not serve the view payload (status {status}): {body}"
    );
}

/// A withdrawn ask and an unknown token: 410 and 404, and the 410 body never commits to WHY.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_dead_link_is_410_and_an_unknown_one_is_404() {
    let (app, _id, token) = rig().await;

    // Unknown, but well-formed: 404, and never a login prompt.
    let res = app
        .clone()
        .oneshot(get(
            "/public/case/request?token=lbr_nube.neverminted",
            "203.0.113.22",
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = body_of(res).await.to_lowercase();
    assert!(
        !body.contains("password") && !body.contains("sign in") && !body.contains("log in"),
        "a dead request link must never offer a login: {body}"
    );

    // Not a request token at all.
    let res = app
        .clone()
        .oneshot(get(
            "/public/case/request?token=lbi_an_invite",
            "203.0.113.22",
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // The live token still resolves — so the two refusals above are about the TOKEN, not the route.
    let res = app
        .clone()
        .oneshot(get(
            &format!("/public/case/request?token={token}"),
            "203.0.113.22",
        ))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "still live before the withdraw"
    );
}

/// The 410 disjunction, driven through a real withdraw on the same node.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_withdrawn_link_is_410_with_a_body_that_never_says_which() {
    let (gw, key) = common::gateway().await;
    let session = common::bootstrap::provision_admin(&gw, "user:test", WS).await;
    let node = gw.node.clone();
    let principal = lb_auth::verify(&key, &session, common::NOW).expect("session");

    let raised = tool(
        &node,
        &principal,
        "insight.raise",
        json!({
            "dedup_key": "gw-withdraw",
            "severity": "warning",
            "title": "probe",
            "origin": { "kind": "rule", "ref": "rule:probe" },
            "ts": NOW_MS,
        }),
    )
    .await;
    let case_id = tool(
        &node,
        &principal,
        "insight.get",
        json!({ "id": raised["id"] }),
    )
    .await["case_id"]
        .as_str()
        .unwrap()
        .to_string();
    tool(
        &node,
        &principal,
        "party.upsert",
        json!({
            "id": "northern",
            "kind": "contractor",
            "name": "Northern",
            "contact": { "email": "ops@northern.example" },
            "default_ask_window_h": 24,
        }),
    )
    .await;
    let request = tool(
        &node,
        &principal,
        "case.request.send",
        json!({
            "case_id": case_id, "party_id": "northern", "ask": "attend", "ts": NOW_MS,
        }),
    )
    .await;

    let provider = Arc::new(RecordingEmailProvider::default());
    let target = EmailTarget::new(Box::new(provider.clone()), node.store.clone());
    relay_outbox(&node.store, WS, &target, common::NOW)
        .await
        .unwrap();
    let raw = token_from(&provider.sends()[0].body);

    tool(
        &node,
        &principal,
        "case.request.withdraw",
        json!({
            "id": request["id"], "ts": NOW_MS + 1,
        }),
    )
    .await;

    let app = router(gw);
    let res = app
        .oneshot(get(
            &format!("/public/case/request?token={raw}"),
            "203.0.113.23",
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::GONE);
    let body = body_of(res).await.to_lowercase();
    // The disjunction, in full. Naming one of the three tells a prober the token was real.
    for word in ["completed", "withdrawn", "timed out"] {
        assert!(
            body.contains(word),
            "the 410 body must stay a disjunction: {body}"
        );
    }
}

/// The routes are rate limited per client IP — they hash a presented secret and answer differently
/// per outcome, which is the same token-oracle posture that put a limiter on the invite route.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_public_case_request_routes_are_rate_limited_per_ip() {
    let (app, _id, _token) = rig().await;
    let ip = "203.0.113.24";

    // Burst with a bogus token: every hit that reaches the handler is a 404, and the (MAX+1)-th is
    // refused before it. The window is wall-clock, so allow one retry round (a broken limiter fails
    // both).
    let mut limited = false;
    for _round in 0..2 {
        for _ in 0..=INVITE_ACCEPT_MAX_PER_WINDOW {
            let res = app
                .clone()
                .oneshot(get("/public/case/request?token=lbr_nube.bogus", ip))
                .await
                .unwrap();
            if res.status() == StatusCode::TOO_MANY_REQUESTS {
                limited = true;
                break;
            }
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
        }
        if limited {
            break;
        }
    }
    assert!(limited, "the ceiling must reject past its limit");

    // A different client is untouched.
    let res = app
        .clone()
        .oneshot(get(
            "/public/case/request?token=lbr_nube.bogus",
            "198.51.100.30",
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
