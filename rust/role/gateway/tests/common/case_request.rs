//! Fixtures for the contractor's public routes (`case_token*_test.rs`).
//!
//! A `tests/common/` sub-module rather than a copy in each file: the rig boots a node, provisions an
//! admin, raises an ask and reads the token back out of the mail that was actually sent, which is
//! ~70 lines nobody should maintain twice. Split out when `case_token_test.rs` passed the 400-line
//! FILE-LAYOUT limit.
#![allow(dead_code)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use lb_host::{call_tool, relay_outbox, EmailTarget, Node, RecordingEmailProvider};
use lb_role_gateway::router;
use serde_json::{json, Value};

pub const WS: &str = "nube";
/// The gateway's clock is SECONDS; the case plane's is millis. `super::NOW` is 1000 s.
pub const NOW_MS: u64 = super::NOW * 1000;

/// Boot a gateway, provision an admin, and raise one ask — returning the router, the request id and
/// the raw token as it appeared in the mail.
pub async fn rig() -> (axum::Router, String, String) {
    let (gw, key) = super::gateway().await;
    let session = super::bootstrap::provision_admin(&gw, "user:test", WS).await;
    let node = gw.node.clone();

    // Seed through the real verbs, under the REAL provisioned admin session — the caps are whatever
    // the durable grants resolve to, not a hand-written list.
    let principal = lb_auth::verify(&key, &session, super::NOW).expect("the admin session");
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
    relay_outbox(&node.store, WS, &target, super::NOW)
        .await
        .expect("a relay pass");
    let sends = provider.sends();
    assert_eq!(sends.len(), 1, "the ask must have mailed a link");
    let raw = token_from(&sends[0].body);

    (router(gw), request_id, raw)
}

pub async fn tool(node: &Arc<Node>, p: &lb_auth::Principal, name: &str, input: Value) -> Value {
    let out = call_tool(node, p, WS, name, &input.to_string())
        .await
        .unwrap_or_else(|e| panic!("{name} failed: {e:?}"));
    serde_json::from_str(&out).unwrap_or(Value::Null)
}

pub fn token_from(body: &str) -> String {
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

pub async fn body_of(res: axum::response::Response) -> String {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8_lossy(&bytes).to_string()
}

pub fn get(uri: &str, ip: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .unwrap()
}

pub fn post_json(uri: &str, ip: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(body.to_string()))
        .unwrap()
}
