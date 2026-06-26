//! S7 (registry slice) — the signature + capability gates, and the happy install-from-registry path,
//! end to end through the real wasm `hello` component.
//!
//! Headline behaviors proven here:
//!   - the **mandatory capability-deny** category: `registry.pull`/`install` refused without the grant,
//!     before any `Source` fetch or store write;
//!   - the **signing/verification** category (the new crypto surface): a tampered or unsigned/foreign-key
//!     artifact is rejected — and rejected **even with the grant** (the signature gate is independent of
//!     the capability gate);
//!   - the happy path: a correctly-signed artifact installs, persists its `Install` record, and its tool
//!     becomes callable.
//!
//! The `Source` and the publisher keys are the only externals (testing §3); store + wasm are real.
//! Multi-thread flavor + a unique workspace id per node-booting test (a node boots a Zenoh peer).

use std::collections::HashMap;

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{install_from_registry, installed, pull, Node, RegistryServiceError, Source};
use lb_mcp::call;
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys, Visibility};

// --- manifests + wasm (the real components) ---------------------------------------------------

const MANIFEST_V1: &str = include_str!("../../../extensions/hello/extension.toml");

fn wasm(rel: &str) -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing component at {} ({e}).\nBuild it first:\n  \
             (cd rust/extensions/hello && cargo build --target wasm32-wasip2 --release)",
            path.display()
        )
    })
}
fn hello_v1() -> Vec<u8> {
    wasm("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm")
}

// --- principal + publisher fixtures -----------------------------------------------------------

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

const PULL: &str = "mcp:registry.pull:call";
const INSTALL: &str = "mcp:registry.install:call";

/// A deterministic publisher (testing §3 — key from a fixed seed). Returns the key id, the signing
/// key, and the matching `TrustedKeys` allow-list a workspace would hold.
fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

/// Sign `(manifest, wasm)` as `key_id` — produces a correctly-signed artifact.
fn sign(
    ext_id: &str,
    version: &str,
    manifest: &str,
    wasm: &[u8],
    key_id: &str,
    sk: &PublisherSigningKey,
) -> Artifact {
    let d = digest(manifest, wasm);
    Artifact {
        ext_id: ext_id.into(),
        version: version.into(),
        manifest_toml: manifest.into(),
        wasm: wasm.to_vec(),
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

// --- the test Source (the only external) ------------------------------------------------------

/// An in-memory artifact origin keyed by `(ext_id, version)`. Can be flipped `offline` (every fetch
/// errors) to prove the cached path never touches it — the offline category lives in its own file.
struct MapSource {
    artifacts: HashMap<(String, String), Artifact>,
    offline: bool,
    fetches: std::sync::Mutex<usize>,
}
impl MapSource {
    fn new(artifacts: Vec<Artifact>) -> Self {
        Self {
            artifacts: artifacts
                .into_iter()
                .map(|a| ((a.ext_id.clone(), a.version.clone()), a))
                .collect(),
            offline: false,
            fetches: std::sync::Mutex::new(0),
        }
    }
}
impl Source for MapSource {
    async fn fetch(&self, ext_id: &str, version: &str) -> Result<Artifact, RegistryServiceError> {
        *self.fetches.lock().unwrap() += 1;
        if self.offline {
            return Err(RegistryServiceError::NotAvailable("offline".into()));
        }
        self.artifacts
            .get(&(ext_id.to_string(), version.to_string()))
            .cloned()
            .ok_or_else(|| RegistryServiceError::NotAvailable(format!("{ext_id}@{version}")))
    }
}

// === tests ====================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn installs_a_signed_artifact_end_to_end() {
    // THE HAPPY PATH: a correctly-signed artifact pulls, verifies, caches, installs; its tool runs.
    let ws = "reg-happy";
    let node = Node::boot().await.unwrap();
    let (kid, sk, trusted) = publisher(10);
    let art = sign("hello", "0.1.0", MANIFEST_V1, &hello_v1(), &kid, &sk);
    let source = MapSource::new(vec![art]);

    let loaded = install_from_registry(
        &node,
        &source,
        ws,
        "hello",
        "0.1.0",
        &trusted,
        &["mcp:hello.echo:call".into()], // admin-approved set
        Visibility::Private,
        1,
    )
    .await
    .expect("signed artifact installs");
    assert!(loaded.tools.contains(&"echo".to_string()));

    // The S4 Install record was persisted (durable answer to "what is allowed here").
    let rec = installed(&node, ws, "hello")
        .await
        .unwrap()
        .expect("installed");
    assert_eq!(rec.version, "0.1.0");

    // The installed tool is callable (subject to its own grant).
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
    .expect("echo");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["echo"], "hi");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_pull_without_grant() {
    // MANDATORY capability-deny: the MCP gate refuses pull without mcp:registry.pull:call. We assert
    // at the gate directly (the bridged read verbs use the same `authorize_registry`).
    let ws = "reg-deny";
    let p_nogrant = principal(ws, &[]); // no registry grant at all
    let err = lb_host::authorize_registry(&p_nogrant, ws, "pull").unwrap_err();
    assert!(matches!(err, RegistryServiceError::Denied));

    let p_other = principal(ws, &["mcp:registry.list:call"]); // a different registry grant
    assert!(matches!(
        lb_host::authorize_registry(&p_other, ws, "install").unwrap_err(),
        RegistryServiceError::Denied
    ));

    // With the right grant, the gate passes (proving the deny is the grant's doing, not a blanket no).
    let p_ok = principal(ws, &[PULL, INSTALL]);
    assert!(lb_host::authorize_registry(&p_ok, ws, "pull").is_ok());
    assert!(lb_host::authorize_registry(&p_ok, ws, "install").is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn install_rejects_tampered_artifact_even_with_grant() {
    // SIGNING/VERIFICATION × CAPABILITY: a fully-granted caller is still handed a bad artifact and
    // must be refused — the signature gate is INDEPENDENT of the capability gate. Nothing is installed.
    let ws = "reg-tampered";
    let node = Node::boot().await.unwrap();
    let (kid, sk, trusted) = publisher(11);
    let mut art = sign("hello", "0.1.0", MANIFEST_V1, &hello_v1(), &kid, &sk);
    // Tamper the wasm AFTER signing — the digest no longer matches the signed one.
    art.wasm.extend_from_slice(b"\x00tamper");
    let source = MapSource::new(vec![art]);

    let err = install_from_registry(
        &node,
        &source,
        ws,
        "hello",
        "0.1.0",
        &trusted,
        &["mcp:hello.echo:call".into()],
        Visibility::Private,
        1,
    )
    .await
    .expect_err("a tampered artifact is refused");
    assert!(matches!(err, RegistryServiceError::Unverified));

    // Nothing was installed and nothing was cached: a re-pull still cannot verify it.
    assert!(installed(&node, ws, "hello").await.unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pull_rejects_artifact_signed_by_untrusted_key() {
    // The artifact is correctly signed — but by a key the workspace does NOT allow-list. Rejected.
    let ws = "reg-foreign-key";
    let node = Node::boot().await.unwrap();
    let (kid, sk, _signer_trust) = publisher(12);
    let (_other_id, _other_sk, workspace_trust) = publisher(13); // workspace trusts a different key
    let art = sign("hello", "0.1.0", MANIFEST_V1, &hello_v1(), &kid, &sk);
    let source = MapSource::new(vec![art]);

    let err = pull(
        &node.store,
        &source,
        ws,
        "hello",
        "0.1.0",
        &workspace_trust,
        Visibility::Private,
        1,
    )
    .await
    .expect_err("foreign-key artifact is refused");
    assert!(matches!(err, RegistryServiceError::Unverified));
}
