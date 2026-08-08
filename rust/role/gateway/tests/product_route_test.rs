//! The `product` object on `GET /node` and `GET /health` (embedder-build-info scope).
//!
//! Run against the REAL gateway over a real booted node (rule 4 — no fake backend). Three
//! properties with consequences, and they are the whole feature:
//!
//! 1. **Absent ⇒ invisible.** With no embedder the two bodies are what they have always been. The
//!    existing key-set assertions in `node_identity_route_test` / `health_route_test` are the other
//!    half of this and deliberately left unchanged: if `product` ever leaked onto a default node
//!    they would fail.
//! 2. **Present ⇒ published on both routes, from ONE source.** The value that reaches `/node` and
//!    the value that reaches `/health` are the same because they read the same cell — the
//!    "two surfaces, one identity" guarantee node-identity established, extended to the build.
//! 3. **`version` is untouched.** The regression test for the rejected rename: `version` keeps
//!    meaning *this crate's* build on both routes, with the product present.
//!
//! **Rule 10:** the fixture product is fabricated (`nube-node`). No lb test may name a real
//! embedder — a grep for `rubix` in this crate stays empty, and that is the point: lb cannot tell
//! which product is on top and nothing here may start.

mod common;

use axum::http::{Request, StatusCode};
use common::*;
use lb_discovery::{BuildInfo, NodeId, NodeIdentity};
use lb_role_gateway::router;
use tower::ServiceExt; // for `oneshot`

/// A fabricated embedder. Deliberately not a real product name — see the module docs.
fn product() -> BuildInfo {
    BuildInfo::new("nube-node", "2.4.0+gdeadbeef1234")
}

fn identity() -> NodeIdentity {
    NodeIdentity::new(NodeId::new("node:gw-01").expect("valid id")).with_name("front office")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn product_is_published_on_node_and_health_from_one_source() {
    let (gw, _key) = gateway().await;
    let gw = gw
        .with_identity(identity(), 8099, vec![])
        .with_build_info(product());

    let node: serde_json::Value =
        json_body(router(gw.clone()).oneshot(get_req("/node")).await.unwrap()).await;
    let health: serde_json::Value =
        json_body(router(gw).oneshot(get_req("/health")).await.unwrap()).await;

    assert_eq!(node["product"]["name"], "nube-node");
    assert_eq!(node["product"]["version"], "2.4.0+gdeadbeef1234");
    // Not merely "both are present" — byte-equal, because they are one value read twice. A fleet
    // tool holding only `/health` must get exactly what a tool holding only `/node` gets.
    assert_eq!(
        node["product"], health["product"],
        "one BuildInfo, two surfaces — they cannot be allowed to disagree"
    );
}

/// The rejected rename, pinned. `version` means the **lb gateway crate** on both routes and keeps
/// meaning it with a product present. Asserted against `CARGO_PKG_VERSION` rather than against
/// "differs from the product" — a difference assertion is a time bomb that goes off the day an
/// embedder coincidentally ships the same number, and would pass for the wrong reason until then.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn version_still_means_lb_with_a_product_present() {
    let (gw, _key) = gateway().await;
    let gw = gw
        .with_identity(identity(), 8099, vec![])
        .with_build_info(product());

    let node: serde_json::Value =
        json_body(router(gw.clone()).oneshot(get_req("/node")).await.unwrap()).await;
    let health: serde_json::Value =
        json_body(router(gw).oneshot(get_req("/health")).await.unwrap()).await;

    assert_eq!(node["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(health["version"], env!("CARGO_PKG_VERSION"));
}

/// No embedder ⇒ the key is **omitted**, never `"product": null`. A null would force every consumer
/// into a two-case read for no gain, and it is the difference between "there is no product" and
/// "there is a product and it is nothing".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_absent_product_is_omitted_not_null() {
    let (gw, _key) = gateway().await;
    // No `with_build_info` — the stock binary's posture, and every existing embedder's.
    let gw = gw.with_identity(identity(), 8099, vec![]);

    let node: serde_json::Value =
        json_body(router(gw.clone()).oneshot(get_req("/node")).await.unwrap()).await;
    let health: serde_json::Value =
        json_body(router(gw).oneshot(get_req("/health")).await.unwrap()).await;

    assert!(
        !node.as_object().unwrap().contains_key("product"),
        "GET /node must omit `product` entirely with no embedder"
    );
    assert!(
        !health.as_object().unwrap().contains_key("product"),
        "GET /health must omit `product` entirely with no embedder"
    );
}

/// `product` is independent of the node identity. A node with no durable identity still answers
/// `/health`, and tying the product to `cfg.identity` would silently drop it on exactly those
/// nodes — the wiring mistake this pins shut.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn health_carries_the_product_even_with_no_node_identity() {
    let (gw, _key) = gateway().await;
    // No `with_identity` at all: `GET /node` 404s, `GET /health` still serves.
    let gw = gw.with_build_info(product());

    let resp = router(gw.clone()).oneshot(get_req("/node")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let health: serde_json::Value =
        json_body(router(gw).oneshot(get_req("/health")).await.unwrap()).await;
    assert_eq!(health["product"]["name"], "nube-node");
}

/// Still unauthenticated, still unaffected by a token — the inherited posture, re-run with the new
/// field present so adding it cannot have quietly moved either route behind the wall.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_garbage_bearer_changes_neither_body() {
    let (gw, _key) = gateway().await;
    let gw = gw
        .with_identity(identity(), 8099, vec![])
        .with_build_info(product());

    for path in ["/node", "/health"] {
        let req = Request::builder()
            .method("GET")
            .uri(path)
            .header("authorization", "Bearer not-a-real-token")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = router(gw.clone()).oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{path} is outside the wall");
        let body: serde_json::Value = json_body(resp).await;
        assert_eq!(body["product"]["name"], "nube-node");
    }
}

/// The key set, with a product present: exactly the old contract **plus** `product`. The
/// unauthenticated-route guard the existing tests draw for the default node, drawn again for the
/// embedder node — so a future "convenient" field on either route trips a test rather than
/// shipping.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_key_set_grows_by_exactly_product() {
    let (gw, _key) = gateway().await;
    let gw = gw
        .with_identity(identity(), 8099, vec![])
        .with_build_info(product());

    let node: serde_json::Value =
        json_body(router(gw.clone()).oneshot(get_req("/node")).await.unwrap()).await;
    let mut keys: Vec<&str> = node
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    // No `machine_id`: this fixture's identity has none, and an absent one is omitted.
    assert_eq!(keys, ["gateway", "name", "node", "product", "version"]);

    let health: serde_json::Value =
        json_body(router(gw).oneshot(get_req("/health")).await.unwrap()).await;
    let mut keys: Vec<&str> = health
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["detail", "product", "status", "version"]);

    let product = node["product"].as_object().expect("product is an object");
    let mut pk: Vec<&str> = product.keys().map(String::as_str).collect();
    pk.sort_unstable();
    assert_eq!(
        pk,
        ["name", "version"],
        "product carries two strings, no more"
    );
}
