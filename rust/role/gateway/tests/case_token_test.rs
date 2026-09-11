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
//!
//! **This half:** the happy path — a token views and replies with no session, and `GET /r/{token}` stays a page.

mod common;

use axum::http::StatusCode;
use serde_json::{json, Value};
use tower::ServiceExt;

use common::case_request::*;

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
