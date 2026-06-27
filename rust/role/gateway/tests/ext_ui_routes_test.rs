//! The gateway's extension-UI surface (ui-federation scope): serving an extension's UI bundle
//! (`GET /extensions/{ext}/ui/{*path}`) and the host-mediated bridge (`POST /mcp/call`). The bundle is
//! non-secret static code (served like any web asset; the token stays in the shell); the bridge
//! re-checks the capability server-side, so an ungranted page is denied exactly like a forged call.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::{router, Gateway};
use tower::ServiceExt;

/// A gateway whose ext-UI dir is a temp fixture holding one bundle at `{dir}/hello-ui/entry.mjs`.
async fn gateway_with_bundle() -> (Gateway, SigningKey, std::path::PathBuf) {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let dir = std::env::temp_dir().join(format!("lb-ext-ui-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("hello-ui")).unwrap();
    std::fs::write(
        dir.join("hello-ui/entry.mjs"),
        b"export function mount(){}\n",
    )
    .unwrap();
    let gw = Gateway::new(node, key.clone(), NOW).with_ext_ui_dir(dir.clone());
    (gw, key, dir)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn serves_an_installed_extension_bundle() {
    let (gw, _key, dir) = gateway_with_bundle().await;
    let resp = router(gw)
        .oneshot(get_req("/extensions/hello-ui/ui/entry.mjs"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ctype = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        ctype.contains("javascript"),
        "ESM served as JS, got {ctype}"
    );
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(body.starts_with(b"export function mount"));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rejects_path_traversal_and_missing_files() {
    let (gw, _key, dir) = gateway_with_bundle().await;
    // Traversal attempt → 400, never escapes the ext dir.
    let resp = router(gw.clone())
        .oneshot(get_req("/extensions/hello-ui/ui/..%2f..%2fsecret"))
        .await
        .unwrap();
    assert!(
        resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::NOT_FOUND,
        "traversal must not 200; got {}",
        resp.status()
    );
    // A file that isn't there → 404.
    let resp = router(gw)
        .oneshot(get_req("/extensions/hello-ui/ui/nope.mjs"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn mcp_bridge_denies_an_ungranted_call() {
    // A page bridges `series.find` but its session holds NO cap → 403 (the host re-check). No token at
    // all → 401. The bundle never gets the token; the bridge can't widen what the session can't do.
    let (gw, key) = gateway().await;
    let body = serde_json::json!({ "tool": "series.find", "args": { "tags": [] } });

    // No bearer → 401.
    let resp = router(gw.clone())
        .oneshot(json_post("/mcp/call", body.clone()))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Authenticated but ungranted → 403.
    let tok = token(&key, "user:page", "acme", &[]);
    let resp = router(gw)
        .oneshot(bearer(json_post("/mcp/call", body), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
