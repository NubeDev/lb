//! `lb ext publish` end to end over a REAL gateway (testing §0): sign a real extension, publish the
//! artifact through the `Remote` transport's `POST /extensions`, and assert the gateway verified +
//! installed it (the tool becomes callable). Also the deny path: a token without `mcp:ext.publish:call`
//! is refused server-side (`403` → honest DENY, exit non-zero), never a fabricated publish.
//!
//! The gateway is seeded to TRUST the CLI's dev publisher key (what `LB_TRUSTED_PUBKEYS` does in the
//! Makefile) — trust is environment, never the upload. The signing key is written under a tempdir
//! `LB_DEVKIT_ROOT`, so the repo stays clean; the env is process-global so this file's tests share one
//! mutex.

mod common;

use std::path::Path;
use std::sync::{Arc, Mutex};

use lb_cli::transport::{ExtPublish, Remote};
use lb_registry::{PublisherKey, TrustedKeys};
use lb_role_gateway::Gateway;

use common::{token, NOW};

static ENV_LOCK: Mutex<()> = Mutex::new(());

const MANIFEST: &str = include_str!("../../../extensions/hello-v2/extension.toml");
const WASM: &[u8] =
    include_bytes!("../../../extensions/hello-v2/target/wasm32-wasip2/release/hello_v2_ext.wasm");

fn stage_devkit_root(root: &Path) {
    let ext = root.join("hello-v2");
    let built = ext.join("target/wasm32-wasip2/release");
    std::fs::create_dir_all(&built).unwrap();
    std::fs::write(ext.join("extension.toml"), MANIFEST).unwrap();
    // `sign` derives the built-binary name from the manifest's `[extension] id` (`hello`) →
    // `hello_ext.wasm` — the built-binary convention. Stage the real wasm bytes under that name so the
    // sign path finds them (the id, not the crate/dir name, drives the lookup).
    std::fs::write(built.join("hello_ext.wasm"), WASM).unwrap();
}

/// The trusted-keys map for the CLI's dev publisher key (the gateway trusts exactly this, as
/// `LB_TRUSTED_PUBKEYS` would).
fn trusted_dev_key() -> TrustedKeys {
    let loaded = lb_devkit::load_or_create_key(&lb_cli::sign::key_path()).unwrap();
    let publisher =
        PublisherKey::from_bytes(&loaded.signing_key.verifying_key().to_bytes()).unwrap();
    let mut trusted = TrustedKeys::new();
    trusted.insert(lb_cli::sign::DEFAULT_KEY_ID.to_string(), publisher);
    trusted
}

/// Spawn a gateway that trusts the CLI's dev key (so a CLI-signed artifact verifies).
async fn spawn_trusting_gateway() -> (
    String,
    Arc<lb_host::Node>,
    lb_auth::SigningKey,
    tokio::task::JoinHandle<()>,
) {
    let node = Arc::new(lb_host::Node::boot_as(lb_host::Role::Hub).await.unwrap());
    let key = lb_auth::SigningKey::generate();
    let gw = Gateway::new(Arc::clone(&node), key.clone(), NOW).with_trusted(trusted_dev_key());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = lb_role_gateway::router(gw);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), node, key, handle)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ext_publish_signs_and_installs_over_the_real_gateway() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    stage_devkit_root(tmp.path());
    std::env::set_var("LB_DEVKIT_ROOT", tmp.path());

    let (base_url, _node, key, handle) = spawn_trusting_gateway().await;
    // A token holding publish + echo + list — the full flow.
    let tok = token(
        &key,
        "user:admin",
        "acme",
        &["mcp:ext.publish:call", "mcp:hello.echo:call"],
    );
    let remote = Remote::new(&base_url, tok);

    // Sign then publish the real hello-v2, exactly as `lb ext publish hello-v2` would.
    let artifact = lb_cli::sign::sign_extension("hello-v2").expect("sign");
    let outcome = remote
        .publish(artifact)
        .await
        .expect("publish over the gateway");
    assert_eq!(outcome, lb_cli::transport::PublishOutcome::Published);

    // The load-bearing assertion: the published component is now CALLABLE (publish installed + loaded
    // it live, not merely cataloged). v2 output carries `v:2`.
    use lb_cli::transport::Transport;
    let out = remote
        .call("hello.echo", serde_json::json!({ "msg": "hi" }))
        .await
        .expect("the loaded tool is callable");
    assert_eq!(out["v"], 2, "the v2 component was loaded: {out}");

    handle.abort();
    std::env::remove_var("LB_DEVKIT_ROOT");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ext_publish_without_the_cap_is_denied_server_side() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    stage_devkit_root(tmp.path());
    std::env::set_var("LB_DEVKIT_ROOT", tmp.path());

    let (base_url, _node, key, handle) = spawn_trusting_gateway().await;
    // A valid session, a fully trusted artifact, but NO ext.publish cap → the server must refuse.
    let tok = token(&key, "user:mallory", "acme", &["bus:chan/*:pub"]);
    let remote = Remote::new(&base_url, tok);

    let artifact = lb_cli::sign::sign_extension("hello-v2").expect("sign");
    match remote.publish(artifact).await {
        Err(lb_cli::error::CliError::Denied { tool }) => assert_eq!(tool, "ext.publish"),
        other => panic!("publish without the cap must be a DENY, got {other:?}"),
    }

    handle.abort();
    std::env::remove_var("LB_DEVKIT_ROOT");
}
