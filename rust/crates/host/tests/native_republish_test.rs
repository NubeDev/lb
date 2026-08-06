//! **Republish a native extension over its own live, running sidecar** — the missing regression
//! test for `write_executable`'s atomic temp-file-then-`rename()` write (lifecycle-management scope,
//! extension-artifact-upload-size fix's verification list).
//!
//! Before commit `a498555e` (2026-07-14), `ext_publish` opened the mapped binary directly for write
//! on a re-publish, which fails `ETXTBSY` ("Text file busy") against a binary a child process is
//! currently executing. `install_dir.rs`'s `write_executable` now writes to a temp sibling and
//! `rename()`s it into place instead — the rename swaps the directory entry without touching the
//! executing inode, so the OLD child keeps running its (now-unlinked) image until `install_native`
//! cooperatively stops it and spawns the new one. That fix has stood since commit `a498555e`, but
//! **no test exercises it against a live, running child** — every existing native-tier test either
//! publishes once (`ext_publish_test.rs`) or republishes only after a restart has already killed the
//! `SidecarMap` (`ext_boot_spawn_test.rs`). This file closes that gap: publish → still running →
//! **republish again, same live node, no restart, no delete** → must succeed and keep answering.
//!
//! No mocks (CLAUDE.md rule #9): the real `echo-sidecar` binary, a real supervised OS child, a real
//! on-disk store.

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_sidecar, ext_publish, Node};
use lb_registry::{
    digest, digest_hex, Artifact, Authenticity, PublisherKey, TrustedKeys, Visibility,
};
use lb_store::Store;
use lb_supervisor::OsLauncher;

const MANIFEST: &str = include_str!("../../../extensions/echo-sidecar/extension.toml");
const PUBLISH: &[&str] = &["mcp:ext.publish:call", "mcp:native.install:call"];

/// The built reference sidecar's bytes — same fixture, same build-first requirement,
/// `ext_boot_spawn_test.rs`'s `sidecar_bytes()`.
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
        wasm: bin.to_vec(),
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

/// The ONE `LB_DIR` this file's tests install native binaries under — see
/// `ext_boot_spawn_test.rs`'s `INSTALL_ROOT` for why this must be set exactly once, never per-test
/// (`LB_DIR` is process-global; libtest runs tests on concurrent threads).
static INSTALL_ROOT: std::sync::LazyLock<std::path::PathBuf> = std::sync::LazyLock::new(|| {
    let dir = std::env::temp_dir().join(format!("lb-native-republish-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("install root");
    std::env::set_var("LB_DIR", &dir);
    dir
});

struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        std::sync::LazyLock::force(&INSTALL_ROOT);
        let dir = std::env::temp_dir().join(format!(
            "lb-native-republish-store-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Scratch(dir)
    }
    fn store(&self) -> String {
        self.0.join("store").to_string_lossy().to_string()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// **The regression test.** Publish once, confirm the child answers, then publish AGAIN over the
/// SAME live node — no restart (`SidecarMap` stays populated), no `DELETE`/uninstall first. Before
/// the atomic-rename fix, the second `ext_publish` would fail trying to open the mapped binary for
/// write while the first child still held it open (`ETXTBSY`). After the fix, it must succeed, and
/// the sidecar must still answer afterward — proving `install_native`'s stop-old-then-spawn-new
/// sequence actually completed, not merely that no error surfaced.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn republishing_over_a_running_native_child_succeeds_and_the_child_still_answers() {
    let scratch = Scratch::new("republish");
    let ws = "republish-native";
    let (kid, sk, trusted) = publisher(41);
    let node = boot_on_path(&scratch.store()).await;
    let caller = principal(ws, PUBLISH);

    // --- first publish: the child spawns and is live. ---
    ext_publish(
        &node,
        &caller,
        ws,
        sign(&sidecar_bytes(), &kid, &sk),
        &trusted,
        Authenticity::Required,
        Visibility::Private,
        1,
    )
    .await
    .expect("first publish spawns the native child");
    assert!(
        node.sidecars.is_running(ws, "echo-sidecar"),
        "the child is running after the first publish"
    );

    let call_caller = principal(ws, &["mcp:echo-sidecar.echo:call", "mcp:native.call:call"]);
    let before = call_sidecar(
        &node,
        &OsLauncher,
        &call_caller,
        ws,
        "echo-sidecar",
        "echo",
        r#""before-republish""#,
        1,
    )
    .await
    .expect("the freshly-published sidecar answers");
    let before: serde_json::Value = serde_json::from_str(&before).unwrap();
    assert_eq!(before["echo"], "before-republish");

    // --- second publish: SAME live node, SAME still-running child, no restart, no delete. This is
    //     the exact sequence that used to ETXTBSY. ---
    ext_publish(
        &node,
        &caller,
        ws,
        sign(&sidecar_bytes(), &kid, &sk),
        &trusted,
        Authenticity::Required,
        Visibility::Private,
        2,
    )
    .await
    .expect(
        "republishing over a running native child must succeed (atomic temp-file-then-rename \
         write, install_dir.rs) — not ETXTBSY",
    );

    // The load-bearing assertion: not just "no error", but a LIVE, ANSWERING child afterward —
    // proving install_native's stop-old-then-spawn-new sequence actually completed the swap.
    assert!(
        node.sidecars.is_running(ws, "echo-sidecar"),
        "a child is still running after republish"
    );
    let after = call_sidecar(
        &node,
        &OsLauncher,
        &call_caller,
        ws,
        "echo-sidecar",
        "echo",
        r#""after-republish""#,
        2,
    )
    .await
    .expect(
        "the sidecar answers again after republish — the swap left a working child, not a corpse",
    );
    let after: serde_json::Value = serde_json::from_str(&after).unwrap();
    assert_eq!(after["echo"], "after-republish");
    assert_eq!(after["ws"], ws, "the scoped identity survives the swap");
}
