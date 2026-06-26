//! S7 (registry slice) — MANDATORY offline category (testing §2, the S7 exit gate): once an artifact
//! is cached, a node installs it with the registry **unreachable**.
//!
//! The proof: pull once (online) to populate the cache, flip the `Source` to offline (every fetch
//! errors and is counted), then install the same `(ext_id, version)` again. It must succeed AND must
//! NOT have called the source — the cached, already-verified bytes are served locally. That is the
//! "once cached, an edge runs offline" guarantee (README §6.4, registry scope).

use std::collections::HashMap;

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_host::{install_from_registry, installed, pull, Node, RegistryServiceError, Source};
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys, Visibility};

const MANIFEST_V1: &str = include_str!("../../../extensions/hello/extension.toml");

fn hello_v1() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("missing hello component at {} ({e})", path.display()))
}

fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

fn sign(version: &str, key_id: &str, sk: &PublisherSigningKey) -> Artifact {
    let wasm = hello_v1();
    let d = digest(MANIFEST_V1, &wasm);
    Artifact {
        ext_id: "hello".into(),
        version: version.into(),
        manifest_toml: MANIFEST_V1.into(),
        wasm,
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

/// The in-memory origin, switchable to offline. Counts fetches so the test can assert "zero source
/// calls on the cached path".
struct MapSource {
    artifacts: HashMap<(String, String), Artifact>,
    offline: std::sync::atomic::AtomicBool,
    fetches: std::sync::atomic::AtomicUsize,
}
impl MapSource {
    fn new(arts: Vec<Artifact>) -> Self {
        Self {
            artifacts: arts
                .into_iter()
                .map(|a| ((a.ext_id.clone(), a.version.clone()), a))
                .collect(),
            offline: std::sync::atomic::AtomicBool::new(false),
            fetches: std::sync::atomic::AtomicUsize::new(0),
        }
    }
    fn go_offline(&self) {
        self.offline
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
    fn fetch_count(&self) -> usize {
        self.fetches.load(std::sync::atomic::Ordering::SeqCst)
    }
}
impl Source for MapSource {
    async fn fetch(&self, ext_id: &str, version: &str) -> Result<Artifact, RegistryServiceError> {
        self.fetches
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.offline.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(RegistryServiceError::NotAvailable("offline".into()));
        }
        self.artifacts
            .get(&(ext_id.to_string(), version.to_string()))
            .cloned()
            .ok_or_else(|| RegistryServiceError::NotAvailable(format!("{ext_id}@{version}")))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pull_serves_cached_bytes_without_source() {
    let ws = "reg-offline-pull";
    let node = Node::boot().await.unwrap();
    let (kid, sk, trusted) = publisher(20);
    let source = MapSource::new(vec![sign("0.1.0", &kid, &sk)]);

    // First pull (online) populates the cache + catalog.
    pull(
        &node.store,
        &source,
        ws,
        "hello",
        "0.1.0",
        &trusted,
        Visibility::Private,
        1,
    )
    .await
    .expect("first pull online");
    assert_eq!(source.fetch_count(), 1);

    // Go offline; the second pull must serve from cache and NOT touch the source.
    source.go_offline();
    pull(
        &node.store,
        &source,
        ws,
        "hello",
        "0.1.0",
        &trusted,
        Visibility::Private,
        2,
    )
    .await
    .expect("cached pull serves offline");
    assert_eq!(
        source.fetch_count(),
        1,
        "cached pull must NOT call the source"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn install_succeeds_offline_once_cached() {
    let ws = "reg-offline-install";
    let node = Node::boot().await.unwrap();
    let (kid, sk, trusted) = publisher(21);
    let source = MapSource::new(vec![sign("0.1.0", &kid, &sk)]);
    let approved = vec!["mcp:hello.echo:call".to_string()];

    // Install once online (caches the verified artifact).
    install_from_registry(
        &node,
        &source,
        ws,
        "hello",
        "0.1.0",
        &trusted,
        &approved,
        Visibility::Private,
        1,
    )
    .await
    .expect("online install");
    assert_eq!(source.fetch_count(), 1);

    // Offline: a fresh node sharing the same store would have the cache; here we re-install on the
    // same node offline. The cached bytes are served — the source is never reached.
    source.go_offline();
    install_from_registry(
        &node,
        &source,
        ws,
        "hello",
        "0.1.0",
        &trusted,
        &approved,
        Visibility::Private,
        2,
    )
    .await
    .expect("offline install from cache");
    assert_eq!(
        source.fetch_count(),
        1,
        "offline install must NOT call the source"
    );
    assert!(installed(&node, ws, "hello").await.unwrap().is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn offline_with_nothing_cached_fails() {
    // The negative: offline AND no cache → NotAvailable. (Proves the cached success above is the
    // cache's doing, not the source silently working.)
    let ws = "reg-offline-cold";
    let node = Node::boot().await.unwrap();
    let (kid, sk, trusted) = publisher(22);
    let source = MapSource::new(vec![sign("0.1.0", &kid, &sk)]);
    source.go_offline();

    let err = pull(
        &node.store,
        &source,
        ws,
        "hello",
        "0.1.0",
        &trusted,
        Visibility::Private,
        1,
    )
    .await
    .expect_err("offline cold pull fails");
    assert!(matches!(err, RegistryServiceError::NotAvailable(_)));
}
