//! Boot bring-up of the **native (Tier-2)** tier — `spawn_enabled` (lifecycle-management scope,
//! issue #64).
//!
//! The bug this pins: `reconcile` planned the native start correctly and `load_enabled` skipped it
//! ("native respawn is the node Launcher's job") — and no node implemented that path. The plan's
//! native actions were computed and dropped, so a published, `enabled: true` native extension never
//! came back after a restart, and **the boot log said nothing at all**. A fully-unimplemented branch
//! sat behind a green suite because the only test asserted the *plan* was right, never that anything
//! *executed* it.
//!
//! So these tests assert the EXECUTION, against a **real supervised OS child** (the reference
//! `echo-sidecar` binary — a real process is the one true external, testing §3) over a **real on-disk
//! store** that outlives the node:
//!   - SURVIVES RESTART: publish a native ext → drop the node (the `SidecarMap` dies with it, exactly
//!     as a process restart kills every child) → re-boot on the same store → `spawn_enabled` brings
//!     the child back, and it ANSWERS a tool call. This is the test that fails without the fix.
//!   - DURABLE INTENT: a `disable`d native install stays down across the restart.
//!   - VISIBILITY: an enabled install whose cached artifact is gone reports `no-cached-artifact`
//!     rather than vanishing silently — the invisibility was half the bug.
//!   - IDEMPOTENCE: a second `spawn_enabled` on a live node does not double-spawn.

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_sidecar, ext_disable, ext_publish, spawn_enabled, Node, SpawnedExt};
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys, Visibility};
use lb_store::Store;
use lb_supervisor::OsLauncher;

const MANIFEST: &str = include_str!("../../../extensions/echo-sidecar/extension.toml");
/// Publishing a NATIVE extension needs BOTH gates: `ext.publish` (the upload) and
/// `native.install` (the spawn `ext_publish` performs for tier=native). The native tier is not
/// special-cased out of its own gate just because the upload path called it.
const PUBLISH: &[&str] = &["mcp:ext.publish:call", "mcp:native.install:call"];

/// The built reference sidecar's bytes — a REAL host binary, published as the artifact payload.
fn sidecar_bytes() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/echo-sidecar");
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing echo-sidecar at {} ({e}).\nBuild it first:\n  (cd rust && cargo build -p echo-sidecar)",
            path.display()
        )
    })
}

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
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

fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

fn sign(bin: &[u8], key_id: &str, sk: &PublisherSigningKey) -> Artifact {
    let d = digest(MANIFEST, bin);
    Artifact {
        ext_id: "echo-sidecar".into(),
        version: "0.1.0".into(),
        manifest_toml: MANIFEST.into(),
        wasm: bin.to_vec(), // the Artifact's payload field carries the native binary for tier=native
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

async fn boot_on_path(path: &str) -> Node {
    Node::boot_with_store(Store::open(path).await.expect("open on-disk store"))
        .await
        .expect("node boots over the on-disk store")
}

/// The ONE `LB_DIR` this whole test binary installs native binaries under.
///
/// `LB_DIR` is process-global and `native_install_dir` reads it live, while libtest runs these tests
/// on concurrent threads — so a per-test `set_var` would be a genuine data race (and, under a leaked
/// value, one test's `Drop` could delete another's binary mid-spawn: green when quiet, red on a busy
/// box). Set it ONCE, before any test can call the reader, and never mutate it again. Isolation comes
/// from the path itself: `native_install_dir` puts `ws` in the path and every test uses its own `ws`,
/// so the subtrees cannot collide.
static INSTALL_ROOT: std::sync::LazyLock<std::path::PathBuf> = std::sync::LazyLock::new(|| {
    let dir = std::env::temp_dir().join(format!("lb-boot-spawn-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("install root");
    std::env::set_var("LB_DIR", &dir);
    dir
});

/// A per-test scratch dir for the STORE. Keyed by test tag + pid, so each test's on-disk store is
/// its own — and its cleanup can never touch another test's files.
struct Scratch {
    dir: std::path::PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Scratch {
        // Force the one-time LB_DIR init before any test body can reach `native_install_dir`.
        std::sync::LazyLock::force(&INSTALL_ROOT);
        let dir = std::env::temp_dir().join(format!("lb-boot-store-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Scratch { dir }
    }
    fn store(&self) -> String {
        self.dir.join("store").to_string_lossy().to_string()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn row<'a>(rows: &'a [SpawnedExt], ext: &str) -> &'a SpawnedExt {
    rows.iter()
        .find(|r| r.ext == ext)
        .unwrap_or_else(|| panic!("no boot-log row for {ext}, got {rows:?}"))
}

/// **The headline: a published native extension survives a node restart.**
///
/// Without `spawn_enabled` wired, this fails at the `spawned` assert: the reconcile plan says
/// `start`, and nothing acts on it — `running=false health=stopped`, forever, exactly as reported.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_published_native_extension_respawns_on_boot_and_answers() {
    let scratch = Scratch::new("survives");
    let ws = "boot-native";
    let (kid, sk, trusted) = publisher(31);
    let art = sign(&sidecar_bytes(), &kid, &sk);

    // --- first boot: publish → the child spawns and is live. ---
    let node1 = boot_on_path(&scratch.store()).await;
    let caller = principal(ws, PUBLISH);
    ext_publish(&node1, &caller, ws, art, &trusted, Visibility::Private, 1)
        .await
        .expect("publish spawns the native child");
    assert!(
        node1.sidecars.is_running(ws, "echo-sidecar"),
        "the child is running after publish (the precondition, not the point)"
    );

    // --- RESTART. Dropping the node drops the SidecarMap — every child dies with the process, and
    //     the durable Install record + the verified cache are all that survive. ---
    drop(node1);

    // --- second boot: a FRESH runtime on the SAME store, with no republish. ---
    let node2 = boot_on_path(&scratch.store()).await;
    assert!(
        !node2.sidecars.is_running(ws, "echo-sidecar"),
        "a fresh node starts with no live children (the restart really happened)"
    );

    let spawned = spawn_enabled(&node2, &OsLauncher, ws, 2)
        .await
        .expect("boot bring-up runs");
    let echo = row(&spawned, "echo-sidecar");
    assert!(
        echo.spawned && echo.reason == "spawned",
        "the enabled native install must be respawned on boot, got {echo:?}"
    );
    assert_eq!(echo.version, "0.1.0", "the boot log names the version");
    assert!(
        node2.sidecars.is_running(ws, "echo-sidecar"),
        "the child process is live again after boot bring-up"
    );

    // The real bar: it does not merely exist, it ANSWERS — the respawned child is reachable through
    // the ordinary MCP call path, with its scoped identity intact.
    let p = principal(ws, &["mcp:echo-sidecar.echo:call", "mcp:native.call:call"]);
    let out = call_sidecar(
        &node2,
        &OsLauncher,
        &p,
        ws,
        "echo-sidecar",
        "echo",
        r#""after-restart""#,
        2,
    )
    .await
    .expect("the respawned sidecar answers a tool call");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["echo"], "after-restart");
    assert_eq!(
        v["ws"], ws,
        "the scoped identity was re-injected on respawn"
    );
}

/// Durable intent outranks the respawn: a DISABLED native install must not come back on boot — the
/// distinction `enable`/`disable` exists for. (The plan half of this was already covered; this pins
/// that the executing half honors it too, which is what the disable promise actually rests on.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_disabled_native_install_stays_down_across_a_restart() {
    let scratch = Scratch::new("disabled");
    let ws = "boot-native-off";
    let (kid, sk, trusted) = publisher(32);
    let art = sign(&sidecar_bytes(), &kid, &sk);

    let node1 = boot_on_path(&scratch.store()).await;
    let admin = principal(ws, &[PUBLISH, &["mcp:ext.disable:call"]].concat());
    ext_publish(&node1, &admin, ws, art, &trusted, Visibility::Private, 1)
        .await
        .expect("publish");
    ext_disable(&node1, &admin, ws, "echo-sidecar", 2)
        .await
        .expect("disable the durable intent");
    drop(node1);

    let node2 = boot_on_path(&scratch.store()).await;
    let spawned = spawn_enabled(&node2, &OsLauncher, ws, 3)
        .await
        .expect("boot bring-up runs");
    let echo = row(&spawned, "echo-sidecar");
    assert!(
        !echo.spawned && echo.reason == "disabled",
        "a disabled native install must NOT silently return after a restart, got {echo:?}"
    );
    assert!(
        !node2.sidecars.is_running(ws, "echo-sidecar"),
        "no child process was spawned for a disabled install"
    );
}

/// The **visibility** half of the bug: an enabled install that cannot be respawned must SAY SO, by
/// name and reason, rather than disappearing. The original failure logged nothing at all, which is
/// what turned a five-minute diagnosis into a multi-hour one.
///
/// Here the durable intent survives but the cached artifact does not (an evicted/pruned cache).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_enabled_install_with_no_cached_artifact_is_reported_not_silent() {
    let scratch = Scratch::new("nocache");
    let ws = "boot-native-nocache";
    let (kid, sk, trusted) = publisher(33);
    let art = sign(&sidecar_bytes(), &kid, &sk);

    let node1 = boot_on_path(&scratch.store()).await;
    let caller = principal(ws, PUBLISH);
    ext_publish(&node1, &caller, ws, art, &trusted, Visibility::Private, 1)
        .await
        .expect("publish");
    // Drop the cached bytes, keeping the (enabled) Install record: the intent to run outlives the
    // artifact. `spawn_enabled` resolves catalog → digest → cache, so an empty cache table is the
    // "bytes are gone" case regardless of how they were evicted.
    node1
        .store
        .query_ws(ws, "DELETE registry_cache", vec![])
        .await
        .expect("evict the artifact cache");
    drop(node1);

    let node2 = boot_on_path(&scratch.store()).await;
    let spawned = spawn_enabled(&node2, &OsLauncher, ws, 3)
        .await
        .expect("boot bring-up does not fail the boot over one extension");
    let echo = row(&spawned, "echo-sidecar");
    assert!(
        !echo.spawned && echo.reason == "no-cached-artifact",
        "an enabled install that cannot respawn must be reported with a reason, got {echo:?}"
    );
}

/// Idempotent against the live runtime: `reconcile` filters already-running children, so a second
/// bring-up is a no-op rather than a second child. (A boot verb that double-spawned would be a new
/// bug in the fix for this one.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_second_bring_up_does_not_double_spawn() {
    let scratch = Scratch::new("idempotent");
    let ws = "boot-native-twice";
    let (kid, sk, trusted) = publisher(34);
    let art = sign(&sidecar_bytes(), &kid, &sk);

    let node = boot_on_path(&scratch.store()).await;
    let caller = principal(ws, PUBLISH);
    ext_publish(&node, &caller, ws, art, &trusted, Visibility::Private, 1)
        .await
        .expect("publish spawns the child");

    // The child is already live, so bring-up must report already-running and leave it alone.
    let spawned = spawn_enabled(&node, &OsLauncher, ws, 2)
        .await
        .expect("bring-up runs");
    let echo = row(&spawned, "echo-sidecar");
    assert!(
        !echo.spawned && echo.reason == "already-running",
        "a running child must not be respawned, got {echo:?}"
    );
    assert!(node.sidecars.is_running(ws, "echo-sidecar"));
}
