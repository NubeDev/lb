//! Shared fixtures for the gateway integration tests (split across `gateway_test.rs` +
//! `gateway_routes_test.rs` to stay under the FILE-LAYOUT 400-line limit). A `tests/common/` module
//! is the standard Rust idiom for sharing test helpers across integration-test binaries — it is NOT
//! itself a test binary. `dead_code` is allowed because each test file uses only a subset.
#![allow(dead_code)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{Node, Role as NodeRole};
use lb_inbox::Item;
use lb_role_gateway::Gateway;

/// The fixed clock the tests mint/verify at.
pub const NOW: u64 = 1000;

/// Build a gateway over a fresh node with a known key (so tests can mint matching tokens).
pub async fn gateway() -> (Gateway, SigningKey) {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    (Gateway::new(node.clone(), key.clone(), NOW), key)
}

/// A gateway sharing a given node (two sessions, one node — the isolation setup).
pub fn gateway_on(node: Arc<Node>, key: &SigningKey) -> Gateway {
    Gateway::new(node, key.clone(), NOW)
}

/// Mint a token signed by `key` for `(sub, ws, caps)`, valid at `NOW`.
pub fn token(key: &SigningKey, sub: &str, ws: &str, caps: &[&str]) -> String {
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: NOW - 1,
        exp: NOW + 10_000,
    };
    mint(key, &claims)
}

pub fn bearer(req: Request<Body>, token: &str) -> Request<Body> {
    let (mut parts, body) = req.into_parts();
    parts
        .headers
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    Request::from_parts(parts, body)
}

pub fn post_req(cid: &str, item: &Item) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/channels/{cid}/messages"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(item).unwrap()))
        .unwrap()
}

pub fn get_req(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

pub fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

pub async fn json_body<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}
