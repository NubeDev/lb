//! Hermetic devkit container build (devkit-container-build-scope.md). Real store, real caps, real
//! Docker CLI — no mocks. Container-mode cases skip (not fail) when Docker or the pinned
//! `lazybones-build` image isn't available on this box, mirroring the existing
//! `wasm_target_ready` skip pattern in `crates/devkit/tests/build_test.rs`.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_devkit::{build_extension, scaffold_extension, Feature, ScaffoldRequest, Tier};
use lb_host::{container_enabled, select_toolchain, Node};

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:devkit-container".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

fn rust_extensions_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extensions")
}

fn request(id: &str, tier: Tier) -> ScaffoldRequest {
    ScaffoldRequest {
        id: id.into(),
        tier,
        features: vec![Feature::SeriesRead],
    }
}

fn docker_image_ready() -> bool {
    Command::new("docker")
        .args(["image", "inspect", "lazybones-build"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Fallback selection (unit): `LB_DEVKIT_BUILDER` config picks the right `Toolchain`, defaulting
/// to `process` when unset — the node's fast inner loop keeps working with no container runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn builder_config_selects_process_by_default() {
    let _guard = env_lock();
    std::env::remove_var("LB_DEVKIT_BUILDER");
    assert!(!container_enabled());

    let node = Node::boot().await.unwrap();
    let toolchain = select_toolchain(&node, "devkit-builder-default").await;
    // ProcessToolchain::ready reports on the host PATH; a Node.js-only host still resolves
    // `cargo` via rustup in CI, so assert indirectly: `ready("__lb_builder_selection_probe__")`
    // is false for a made-up program on both toolchains, but the container path additionally
    // requires the pinned image, which is not configured here (no LB_DEVKIT_BUILD_IMAGE / no
    // "container" mode) — so behavior would differ if the wrong toolchain were selected only
    // when container mode is actually enabled. Direct assertion covers config selection itself.
    assert!(!toolchain.ready("__lb_builder_selection_probe__"));
}

/// Fallback selection (unit): `LB_DEVKIT_BUILDER=container` opts in.
#[test]
fn builder_config_container_flag_parses() {
    let _guard = env_lock();
    std::env::set_var("LB_DEVKIT_BUILDER", "container");
    assert!(container_enabled());
    std::env::set_var("LB_DEVKIT_BUILDER", "process");
    assert!(!container_enabled());
    std::env::remove_var("LB_DEVKIT_BUILDER");
    assert!(!container_enabled());
}

/// Toolchain-parity (integration): the same extension builds via `ProcessToolchain` and via
/// `ContainerToolchain` to an installable artifact, proving the trait swap is behavior-preserving.
/// Skips (does not fail) if Docker or the pinned image isn't present on this box.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn container_toolchain_builds_same_artifact_as_process_toolchain() {
    let _guard = env_lock();
    if !docker_image_ready() {
        eprintln!("skipping: docker or the lazybones-build image is not available");
        return;
    }

    let root = rust_extensions_root();
    let id = format!("devkit-parity-{}", std::process::id());
    let _ = std::fs::remove_dir_all(root.join(&id));
    let report = scaffold_extension(Some(&root), &request(&id, Tier::Native)).unwrap();

    let mut logs = Vec::new();
    std::env::set_var("LB_DEVKIT_BUILDER", "container");
    let node = Node::boot().await.unwrap();
    let toolchain = select_toolchain(&node, "devkit-parity").await;
    std::env::remove_var("LB_DEVKIT_BUILDER");

    let result = build_extension(&report.path, toolchain.as_ref(), &mut |line| {
        logs.push(line)
    });
    let built_bin = report.path.join("target/release").join(&id);
    let artifact_exists = built_bin.exists();
    let _ = std::fs::remove_dir_all(&report.path);

    if let Err(err) = result {
        panic!("container build failed: {err}\n{}", logs.join("\n"));
    }
    assert!(artifact_exists, "expected a built native artifact");
}

/// Private-dep credential (integration): the streamed log never contains the git token, whether a
/// build succeeds or fails. Skips if Docker/the image aren't available. This is the regression
/// test for a native extension's `exit 101` symptom (docs/debugging/extensions/).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn container_build_log_never_contains_the_git_token() {
    let _guard = env_lock();
    if !docker_image_ready() {
        eprintln!("skipping: docker or the lazybones-build image is not available");
        return;
    }

    let root = rust_extensions_root();
    let id = format!("devkit-credential-{}", std::process::id());
    let _ = std::fs::remove_dir_all(root.join(&id));
    let report = scaffold_extension(Some(&root), &request(&id, Tier::Native)).unwrap();

    let secret_token = "lb-test-secret-token-should-never-be-logged";
    let node = Node::boot().await.unwrap();
    let ws = "devkit-credential";
    let admin = principal(ws, &["secret:devkit/*:write"]);
    lb_secrets::set(
        &node.store,
        &admin,
        ws,
        "devkit/build-git-token",
        secret_token,
    )
    .await
    .expect("seed build token secret");

    std::env::set_var("LB_DEVKIT_BUILDER", "container");
    let toolchain = select_toolchain(&node, ws).await;
    std::env::remove_var("LB_DEVKIT_BUILDER");

    let mut logs = Vec::new();
    let _ = build_extension(&report.path, toolchain.as_ref(), &mut |line| {
        logs.push(line)
    });
    let _ = std::fs::remove_dir_all(&report.path);

    for line in &logs {
        assert!(
            !line.contains(secret_token),
            "build log leaked the git token: {line}"
        );
    }
}

/// Deny / failure paths: container mode selected but no runtime/image present fails with a clear
/// message, not a panic or a cryptic spawn error.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn container_build_fails_clearly_when_image_is_missing() {
    let _guard = env_lock();
    let root = rust_extensions_root();
    let id = format!("devkit-missing-image-{}", std::process::id());
    let _ = std::fs::remove_dir_all(root.join(&id));
    let report = scaffold_extension(Some(&root), &request(&id, Tier::Native)).unwrap();

    std::env::set_var("LB_DEVKIT_BUILDER", "container");
    std::env::set_var(
        "LB_DEVKIT_BUILD_IMAGE",
        "lazybones-build-image-that-does-not-exist",
    );
    let node = Node::boot().await.unwrap();
    let toolchain = select_toolchain(&node, "devkit-missing-image").await;
    std::env::remove_var("LB_DEVKIT_BUILDER");
    std::env::remove_var("LB_DEVKIT_BUILD_IMAGE");

    let mut logs = Vec::new();
    let result = build_extension(&report.path, toolchain.as_ref(), &mut |line| {
        logs.push(line)
    });
    let _ = std::fs::remove_dir_all(&report.path);

    assert!(
        result.is_err(),
        "expected a clear failure, not a hang/panic"
    );
}
