//! `ext.publish` — the upload-then-**install-and-load** path (lifecycle-management scope: the gap where
//! publish stopped at the catalog and nothing brought the component online). The browser's
//! `POST /extensions` and the dev `lb-pack` packager both feed this verb a signed [`Artifact`].
//!
//! Headline behaviors proven here, all through the **real wasm `hello` component** (no mocks — §9):
//!   - HAPPY PATH: a correctly-signed artifact publishes, persists its `Install` record, AND becomes
//!     **callable immediately** — publish now loads, it does not merely catalog.
//!   - MANDATORY capability-deny: `ext.publish` is refused without `mcp:ext.publish:call`, and nothing
//!     is stored or loaded.
//!   - VERIFICATION × CAPABILITY: a fully-granted caller handing over a tampered artifact is still
//!     refused (`Unverified`) — the signature gate is independent of the capability gate; nothing loads.
//!   - SURVIVES RESTART: `load_enabled` re-loads the published-and-enabled extension from the durable
//!     verified cache into a fresh runtime, so an upload outlives a node restart.
//!
//! Workspace-first throughout (the caller's token carries the ws). The publisher key is the only
//! external (testing §3); store + wasm + runtime are real.

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{ext_disable, ext_publish, installed, load_enabled, ExtError, Node};
use lb_mcp::call;
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys, Visibility};
use lb_store::Store;

const MANIFEST_V2: &str = include_str!("../../../extensions/hello-v2/extension.toml");

fn hello_v2() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello-v2/target/wasm32-wasip2/release/hello_v2_ext.wasm");
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing component at {} ({e}).\nBuild it first:\n  bash rust/extensions/hello-v2/build.sh",
            path.display()
        )
    })
}

const PUBLISH: &str = "mcp:ext.publish:call";

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A deterministic dev publisher (key from a fixed seed) + the matching trust allow-list.
fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

fn sign(manifest: &str, wasm: &[u8], key_id: &str, sk: &PublisherSigningKey) -> Artifact {
    let d = digest(manifest, wasm);
    Artifact {
        ext_id: "hello".into(),
        version: "0.2.0".into(),
        manifest_toml: manifest.into(),
        wasm: wasm.to_vec(),
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publish_installs_and_loads_the_extension_callable() {
    let ws = "pub-happy";
    let node = Node::boot().await.unwrap();
    let (kid, sk, trusted) = publisher(20);
    let art = sign(MANIFEST_V2, &hello_v2(), &kid, &sk);

    let caller = principal(ws, &[PUBLISH]);
    ext_publish(&node, &caller, ws, art, &trusted, Visibility::Private, 1)
        .await
        .expect("a signed artifact publishes-and-installs");

    // The durable Install record exists (publish now installs).
    let rec = installed(&node, ws, "hello")
        .await
        .unwrap()
        .expect("installed");
    assert_eq!(rec.version, "0.2.0");

    // And the tool is callable RIGHT NOW — publish loaded the component, it did not merely catalog it.
    let p = principal(ws, &["mcp:hello.echo:call"]);
    let out = call(
        &node.registry,
        &node.bus,
        &p,
        ws,
        "hello.echo",
        r#"{"msg":"hi"}"#,
    )
    .await
    .expect("echo on the just-published extension");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["echo"], "hi");
    assert_eq!(v["v"], 2, "the v2 component is the one that loaded");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publish_is_denied_without_the_grant_and_nothing_is_stored() {
    let ws = "pub-deny";
    let node = Node::boot().await.unwrap();
    let (kid, sk, trusted) = publisher(21);
    let art = sign(MANIFEST_V2, &hello_v2(), &kid, &sk);

    let caller = principal(ws, &[]); // no ext.publish grant
    let err = ext_publish(&node, &caller, ws, art, &trusted, Visibility::Private, 1)
        .await
        .expect_err("publish without the grant is denied");
    assert!(matches!(err, ExtError::Denied));
    assert!(
        installed(&node, ws, "hello").await.unwrap().is_none(),
        "a denied publish stores nothing"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publish_rejects_a_tampered_artifact_even_with_the_grant() {
    let ws = "pub-tampered";
    let node = Node::boot().await.unwrap();
    let (kid, sk, trusted) = publisher(22);
    let mut art = sign(MANIFEST_V2, &hello_v2(), &kid, &sk);
    art.wasm.extend_from_slice(b"\x00tamper"); // digest no longer matches the signature

    let caller = principal(ws, &[PUBLISH]); // fully granted, still refused — the gates are independent
    let err = ext_publish(&node, &caller, ws, art, &trusted, Visibility::Private, 1)
        .await
        .expect_err("a tampered artifact is refused");
    assert!(matches!(err, ExtError::Unverified));
    assert!(
        installed(&node, ws, "hello").await.unwrap().is_none(),
        "nothing is stored or loaded on a verification failure"
    );
}

/// Boot a node on an explicit on-disk store `path` WITHOUT touching the global `LB_STORE_PATH`
/// env (which would race the other tests in this shared binary). Same wiring as `Node::boot`, just a
/// store handle we control — so we can open node1, drop it, and re-open the same bytes as node2: a
/// real restart, in-process and race-free.
async fn boot_on_path(path: &str) -> Node {
    // A custom on-disk store (durable across the restart this test asserts) + the default peer bus +
    // Solo role — exactly `boot_with_store`, which also installs the encapsulated runtime registry.
    Node::boot_with_store(Store::open(path).await.expect("open on-disk store"))
        .await
        .expect("node boots over the on-disk store")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn published_extension_survives_a_restart_via_load_enabled() {
    // The durable Install record + the digest-keyed verified cache are the source of truth. A node
    // restart re-loads from them: same store path, a fresh runtime, `load_enabled` brings it back.
    let dir = std::env::temp_dir().join(format!("lb-pub-restart-{}", std::process::id()));
    let path = dir.to_string_lossy().to_string();
    let _ = std::fs::remove_dir_all(&dir);

    let ws = "pub-restart";
    let (kid, sk, trusted) = publisher(23);
    let art = sign(MANIFEST_V2, &hello_v2(), &kid, &sk);

    // --- first boot: publish (installs + loads + caches the verified bytes) ---
    let node1 = boot_on_path(&path).await;
    let caller = principal(ws, &[PUBLISH]);
    ext_publish(&node1, &caller, ws, art, &trusted, Visibility::Private, 1)
        .await
        .expect("publish on first boot");
    drop(node1); // release the store handle before re-opening the same path

    // --- second boot: a FRESH runtime on the SAME store. load_enabled re-loads from the cache. ---
    let node2 = boot_on_path(&path).await;
    let loaded = load_enabled(&node2, ws).await.expect("reconcile + load");
    assert!(
        loaded.iter().any(|e| e.ext == "hello" && e.loaded),
        "the enabled wasm install is re-loaded on boot, got {loaded:?}"
    );

    // The tool works on the fresh runtime — the extension truly survived the restart.
    let p = principal(ws, &["mcp:hello.echo:call"]);
    let out = call(
        &node2.registry,
        &node2.bus,
        &p,
        ws,
        "hello.echo",
        r#"{"msg":"again"}"#,
    )
    .await
    .expect("echo after restart");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["echo"], "again");

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_disabled_install_is_not_brought_back_by_load_enabled() {
    // `disable` is durable intent distinct from running: the boot reconciler must honor it, so a
    // disabled extension does NOT silently return after a restart. We publish (enabled), then disable,
    // then assert `load_enabled` reports it skipped ("disabled"), never loaded.
    let dir = std::env::temp_dir().join(format!("lb-pub-disabled-{}", std::process::id()));
    let path = dir.to_string_lossy().to_string();
    let _ = std::fs::remove_dir_all(&dir);

    let ws = "pub-disabled";
    let (kid, sk, trusted) = publisher(24);
    let art = sign(MANIFEST_V2, &hello_v2(), &kid, &sk);

    let node1 = boot_on_path(&path).await;
    let admin = principal(ws, &[PUBLISH, "mcp:ext.disable:call"]);
    ext_publish(&node1, &admin, ws, art, &trusted, Visibility::Private, 1)
        .await
        .expect("publish");
    ext_disable(&node1, &admin, ws, "hello", 2)
        .await
        .expect("disable the durable intent");
    drop(node1);

    // Fresh runtime, same store: load_enabled honors the disabled intent — nothing is loaded.
    let node2 = boot_on_path(&path).await;
    let loaded = load_enabled(&node2, ws).await.expect("reconcile");
    assert!(
        loaded.iter().all(|e| !(e.ext == "hello" && e.loaded)),
        "a disabled install is not brought back, got {loaded:?}"
    );
    assert!(
        loaded
            .iter()
            .any(|e| e.ext == "hello" && e.reason == "disabled"),
        "and it is reported as skipped-because-disabled, got {loaded:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
