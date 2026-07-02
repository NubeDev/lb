//! `lb devkit sign` produces an artifact the registry's `verify_artifact` ACCEPTS (operator-cli scope:
//! the `lb-pack` fold). The oracle is `verify_artifact` itself over the dev key the sign used — the
//! same `lb-devkit` library `lb-pack` uses, so the artifact is byte-identical to `lb-pack`'s and
//! verifies by construction. No mocks: a REAL built wasm (`hello-v2`) is signed and verified.
//!
//! The test is hermetic: it lays out a throwaway devkit root (`{root}/hello-v2/…`) from the real
//! manifest + built wasm, points `LB_DEVKIT_ROOT` at it, and signs — so the dev key is written under
//! the tempdir, never the repo. Env vars are process-global, so the two env-touching tests here share
//! one mutex.

use std::path::Path;
use std::sync::Mutex;

use lb_registry::{verify_artifact, PublisherKey, TrustedKeys};

/// Serialize the env-mutating tests (LB_DEVKIT_ROOT is process-global).
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// The real built inputs for `hello-v2` (same files the publish_install gateway test uses).
const MANIFEST: &str = include_str!("../../../extensions/hello-v2/extension.toml");
const WASM: &[u8] =
    include_bytes!("../../../extensions/hello-v2/target/wasm32-wasip2/release/hello_v2_ext.wasm");

/// Lay out a throwaway devkit root with `hello-v2` built, and return its path. The tier is `wasm`, so
/// the built binary lives at `target/wasm32-wasip2/release/hello_v2_ext.wasm` (what `sign` looks for).
fn stage_devkit_root(root: &Path) {
    let ext = root.join("hello-v2");
    let built = ext.join("target/wasm32-wasip2/release");
    std::fs::create_dir_all(&built).unwrap();
    std::fs::write(ext.join("extension.toml"), MANIFEST).unwrap();
    // `sign` derives the built-binary name from the manifest's `[extension] id` (`hello`) →
    // `hello_ext.wasm`. Stage the real wasm bytes under that name so the sign path finds them.
    std::fs::write(built.join("hello_ext.wasm"), WASM).unwrap();
}

#[test]
fn devkit_sign_produces_an_artifact_verify_artifact_accepts() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    stage_devkit_root(tmp.path());
    std::env::set_var("LB_DEVKIT_ROOT", tmp.path());

    // Sign the real extension.
    let artifact = lb_cli::sign::sign_extension("hello-v2").expect("sign hello-v2");
    assert_eq!(artifact.ext_id, "hello");
    assert_eq!(artifact.version, "0.2.0");

    // The oracle: build the trusted-keys map from the dev key the sign just used, and verify. If the
    // digest or signature were off, this would reject — this asserts the round-trip the scope requires.
    let loaded = lb_devkit::load_or_create_key(&lb_cli::sign::key_path()).unwrap();
    let publisher =
        PublisherKey::from_bytes(&loaded.signing_key.verifying_key().to_bytes()).unwrap();
    let mut trusted = TrustedKeys::new();
    trusted.insert(lb_cli::sign::DEFAULT_KEY_ID.to_string(), publisher);

    verify_artifact(artifact, &trusted).expect("the signed artifact verifies (the lb-pack oracle)");

    std::env::remove_var("LB_DEVKIT_ROOT");
}

#[test]
fn a_tampered_artifact_fails_verification() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    stage_devkit_root(tmp.path());
    std::env::set_var("LB_DEVKIT_ROOT", tmp.path());

    let mut artifact = lb_cli::sign::sign_extension("hello-v2").expect("sign");
    // Flip a wasm byte AFTER signing — the digest binds the bytes, so verify must reject (nothing an
    // operator can tamper survives the registry gate).
    artifact.wasm.push(0xff);

    let loaded = lb_devkit::load_or_create_key(&lb_cli::sign::key_path()).unwrap();
    let publisher =
        PublisherKey::from_bytes(&loaded.signing_key.verifying_key().to_bytes()).unwrap();
    let mut trusted = TrustedKeys::new();
    trusted.insert(lb_cli::sign::DEFAULT_KEY_ID.to_string(), publisher);

    assert!(
        verify_artifact(artifact, &trusted).is_err(),
        "a tampered artifact must fail verification"
    );

    std::env::remove_var("LB_DEVKIT_ROOT");
}
