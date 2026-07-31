//! `GET /node` — the unauthenticated node-identity probe, end to end (node-identity scope).
//!
//! Run against the REAL gateway over a real booted node (rule 4 — no fake backend). What these pin
//! is the part with consequences: the route sits OUTSIDE the auth wall, so it must publish identity
//! and reachability and **nothing behind a wall**, and it must not invent an identity for a node
//! that has no durable one.

mod common;

use std::net::{IpAddr, Ipv4Addr};

use axum::http::{Request, StatusCode};
use common::*;
use lb_discovery::{NodeId, NodeIdentity};
use lb_role_gateway::router;
use tower::ServiceExt; // for `oneshot`

fn identity() -> NodeIdentity {
    NodeIdentity::new(NodeId::new("node:gw-01").expect("valid id"))
        .with_name("front office")
        .with_machine_id("mid-abc123")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn node_identity_is_served_unauthenticated() {
    let (gw, _key) = gateway().await;
    let gw = gw.with_identity(
        identity(),
        8099,
        vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 40))],
    );

    // Bare GET, NO Authorization header — the route is outside the auth wall, like `/health`.
    let resp = router(gw).oneshot(get_req("/node")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = json_body(resp).await;

    assert_eq!(body["node"], "node:gw-01");
    assert_eq!(body["name"], "front office");
    assert_eq!(body["machine_id"], "mid-abc123");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(body["gateway"]["port"], 8099);
    assert_eq!(body["gateway"]["addresses"][0], "192.168.1.40");
}

/// The wall (rule 6), asserted on the response: the body's key set is EXACTLY the identity +
/// reachability contract. This fails the moment someone adds a "convenient" field — a workspace, a
/// member, an extension list — to a route that anything on the network can read without a token.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn node_identity_leaks_nothing_beyond_the_contract() {
    let (gw, _key) = gateway().await;
    let gw = gw.with_identity(identity(), 8099, vec![]);
    let resp = router(gw).oneshot(get_req("/node")).await.unwrap();
    let body: serde_json::Value = json_body(resp).await;

    let obj = body.as_object().expect("body is an object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["gateway", "machine_id", "name", "node", "version"],
        "GET /node must publish identity + reachability and NOTHING else"
    );
    let gateway_obj = obj["gateway"].as_object().expect("gateway is an object");
    let mut gwk: Vec<&str> = gateway_obj.keys().map(String::as_str).collect();
    gwk.sort_unstable();
    assert_eq!(gwk, ["addresses", "port"]);
}

/// No configured identity ⇒ `404`, NOT a body carrying the per-boot random id. Publishing that
/// would hand a caller an "identity" that silently changes on the next restart.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn node_identity_404s_when_none_is_configured() {
    let (gw, _key) = gateway().await;
    // No `with_identity` — the default posture for every existing node.
    let resp = router(gw).oneshot(get_req("/node")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// A garbage bearer must not change the answer — the route never reaches the auth wall, so it
/// neither honours nor rejects a token. (Same property `/health` pins.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn node_identity_ignores_a_stale_bearer() {
    let (gw, _key) = gateway().await;
    let gw = gw.with_identity(identity(), 8099, vec![]);
    let req = Request::builder()
        .method("GET")
        .uri("/node")
        .header("authorization", "Bearer not-a-real-token")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = router(gw).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

/// An absent machine id is OMITTED from the body, never serialized as `null`. A client can then
/// treat "key present" as "this node has a machine-derived id", with no null-vs-missing ambiguity.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_absent_machine_id_is_omitted_not_null() {
    let (gw, _key) = gateway().await;
    let bare = NodeIdentity::new(NodeId::new("node:gw-02").expect("valid id"));
    let gw = gw.with_identity(bare, 8099, vec![]);
    let resp = router(gw).oneshot(get_req("/node")).await.unwrap();
    let body: serde_json::Value = json_body(resp).await;

    assert!(
        !body.as_object().unwrap().contains_key("machine_id"),
        "an absent machine id must not appear as null"
    );
    // `name` still arrives — it defaults to the node id rather than being blank.
    assert_eq!(body["name"], "node:gw-02");
}
