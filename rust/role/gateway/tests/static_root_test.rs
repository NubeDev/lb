//! The gateway's optional static-root fallback (static-root scope): when an embedder pins a static
//! web-app dir via `BootConfig::static_root` / `Gateway::with_static_root`, the router serves that tree
//! at `/` for every request matching no API/ext-UI route — `/` → `index.html`, a real asset by its
//! path, and an unknown deep link → `index.html` (so a browser-router SPA boots and routes client-side).
//! Unset ⇒ no fallback, unmatched paths 404 exactly as before. Generic: the gateway never learns whose
//! app it is (rule 10). This is the seam a self-contained single-binary build (ARM/Pi) serves its shell
//! from.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::{router, Gateway};
use tower::ServiceExt;

/// A gateway whose static root is a temp fixture holding an `index.html` + one asset.
///
/// NOTE the asset lives under `/app`, not `/assets` — the gateway has a real `/assets/{id}` API route
/// (`get_asset_bin`) that WINS over the fallback (as `api_routes_win_over_the_fallback` asserts). A real
/// embedder's bundler must therefore emit its assets under a non-colliding dir (e.g. Vite
/// `build.assetsDir`), or those files would 401 instead of being served. This is a deploy contract, not
/// an lb bug: API routes intentionally take precedence over the static fallback.
async fn gateway_with_static_root(tag: &str) -> (Gateway, std::path::PathBuf) {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    // Per-test dir (pid + a test-unique tag) so parallel tests never race on cleanup.
    let dir = std::env::temp_dir().join(format!("lb-static-root-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(dir.join("app")).unwrap();
    std::fs::write(
        dir.join("index.html"),
        b"<!doctype html><title>shell</title>\n",
    )
    .unwrap();
    std::fs::write(dir.join("app/app.js"), b"console.log('app')\n").unwrap();
    let gw = Gateway::new(node, key, NOW).with_static_root(dir.clone());
    (gw, dir)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn serves_index_at_root() {
    let (gw, dir) = gateway_with_static_root("index").await;
    let resp = router(gw).oneshot(get_req("/")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(
        body.starts_with(b"<!doctype html>"),
        "`/` serves index.html"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn serves_a_real_asset_by_path() {
    let (gw, dir) = gateway_with_static_root("asset").await;
    let resp = router(gw).oneshot(get_req("/app/app.js")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(body.starts_with(b"console.log"), "the real asset is served");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deep_link_falls_back_to_index_for_the_spa_router() {
    // A client-side route with no matching file must serve index.html (200), not 404 — otherwise a
    // refresh on `/sites/123` would break the SPA.
    let (gw, dir) = gateway_with_static_root("deeplink").await;
    let resp = router(gw)
        .oneshot(get_req("/sites/deep/link"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(
        body.starts_with(b"<!doctype html>"),
        "deep link → index.html"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn api_routes_win_over_the_fallback() {
    // The fallback must not shadow the API surface: an unauthenticated `POST /mcp/call` is still the
    // API's 401, never the static index.
    let (gw, dir) = gateway_with_static_root("apiwins").await;
    let body = serde_json::json!({ "tool": "series.find", "args": { "tags": [] } });
    let resp = router(gw)
        .oneshot(json_post("/mcp/call", body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn no_static_root_means_unmatched_paths_404() {
    // The default (no static root) is unchanged: an unmatched path is a 404, not a fallback.
    let (gw, _key) = gateway().await;
    let resp = router(gw)
        .oneshot(get_req("/some/unknown/path"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
