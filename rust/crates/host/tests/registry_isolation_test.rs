//! S7 (registry slice) — MANDATORY workspace-isolation category (testing §2, README §7): a
//! workspace's registry cache and catalog are invisible to another workspace, across **store + MCP**.
//!
//! Two surfaces:
//!   - **store:** ws-A pulls + caches a private artifact; ws-B `resolve`/`read_cached` returns `None`
//!     (the cache/catalog live in ws-A's namespace — the hard wall is structural), and ws-B's pull of
//!     the same version while the source is OFFLINE fails (ws-B has nothing cached — it cannot ride
//!     ws-A's cache).
//!   - **MCP:** a ws-B principal is refused at the `registry.*` gate for ws-A (workspace-first), and a
//!     ws-B caller cannot `list` ws-A's catalog entries through the bridge.

use std::collections::HashMap;

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_registry_tool, pull, read_cached, resolve_catalog, Node, RegistryServiceError, Source,
};
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys, Visibility};
use serde_json::json;

const MANIFEST_V1: &str = include_str!("../../../extensions/hello/extension.toml");

fn hello_v1() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("missing hello component at {} ({e})", path.display()))
}

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

fn sign(key_id: &str, sk: &PublisherSigningKey) -> Artifact {
    let wasm = hello_v1();
    let d = digest(MANIFEST_V1, &wasm);
    Artifact {
        ext_id: "hello".into(),
        version: "0.1.0".into(),
        manifest_toml: MANIFEST_V1.into(),
        wasm,
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

struct MapSource {
    artifacts: HashMap<(String, String), Artifact>,
    offline: std::sync::atomic::AtomicBool,
}
impl MapSource {
    fn new(arts: Vec<Artifact>) -> Self {
        Self {
            artifacts: arts
                .into_iter()
                .map(|a| ((a.ext_id.clone(), a.version.clone()), a))
                .collect(),
            offline: std::sync::atomic::AtomicBool::new(false),
        }
    }
    fn go_offline(&self) {
        self.offline
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
impl Source for MapSource {
    async fn fetch(&self, ext_id: &str, version: &str) -> Result<Artifact, RegistryServiceError> {
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
async fn ws_b_cannot_see_ws_a_cache_or_catalog_in_store() {
    let (ws_a, ws_b) = ("reg-iso-a", "reg-iso-b");
    let node = Node::boot().await.unwrap();
    let (kid, sk, trusted) = publisher(40);
    let source = MapSource::new(vec![sign(&kid, &sk)]);

    // ws-A pulls + caches the private artifact.
    pull(
        &node.store,
        &source,
        ws_a,
        "hello",
        "0.1.0",
        &trusted,
        Visibility::Private,
        1,
    )
    .await
    .expect("ws-A pull");

    // STORE isolation: ws-B sees neither the catalog entry nor the cached bytes.
    assert!(
        resolve_catalog(&node.store, ws_b, "hello", "0.1.0")
            .await
            .unwrap()
            .is_none(),
        "ws-B must not resolve ws-A's private catalog entry"
    );
    // ws-A's cache key (the digest) is known; ws-B still cannot read it from its own namespace.
    let digest_hex_a = resolve_catalog(&node.store, ws_a, "hello", "0.1.0")
        .await
        .unwrap()
        .unwrap()
        .digest_hex;
    assert!(
        read_cached(&node.store, ws_b, &digest_hex_a)
            .await
            .unwrap()
            .is_none(),
        "ws-B must not read ws-A's cached artifact bytes"
    );

    // And ws-B cannot ride ws-A's cache: offline, with nothing cached of its own, its pull fails.
    source.go_offline();
    let err = pull(
        &node.store,
        &source,
        ws_b,
        "hello",
        "0.1.0",
        &trusted,
        Visibility::Private,
        2,
    )
    .await
    .expect_err("ws-B offline pull has no cache to ride");
    assert!(matches!(err, RegistryServiceError::NotAvailable(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_principal_is_denied_at_the_registry_gate_for_ws_a() {
    // MCP isolation, workspace-first: a principal scoped to ws-B is refused for ws-A even WITH the
    // registry grant, because the workspace claim mismatches (gate 1 before gate 2).
    let (ws_a, ws_b) = ("reg-iso-mcp-a", "reg-iso-mcp-b");
    let node = Node::boot().await.unwrap();

    // The principal carries ws-B's claim and the list grant.
    let p_b = principal("user:b", ws_b, &["mcp:registry.list:call"]);

    // Calling the bridge for ws-A with a ws-B principal is denied at the MCP gate (no catalog leaked).
    let err = call_registry_tool(
        &node,
        &p_b,
        ws_a,
        "registry.list",
        &json!({ "ext_id": "hello" }),
    )
    .await
    .expect_err("ws-B principal denied for ws-A");
    assert_eq!(err, lb_mcp::ToolError::Denied);

    // For its OWN workspace the same principal is authorized (empty catalog, but no denial).
    let out = call_registry_tool(
        &node,
        &p_b,
        ws_b,
        "registry.list",
        &json!({ "ext_id": "hello" }),
    )
    .await
    .expect("ws-B may list its own catalog");
    assert_eq!(out["entries"].as_array().unwrap().len(), 0);
}
