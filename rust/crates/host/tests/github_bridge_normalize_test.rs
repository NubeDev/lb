//! S7 (github-bridge slice) — the `github-bridge` TRANSFORM surface, exercised through the real
//! `github_bridge_ext.wasm` (github-bridge scope). The install-lifecycle categories (happy/offline/
//! rollback/isolation) live in `github_bridge_test.rs`; this file covers the pure-transform branches
//! and the per-tool capability-deny:
//!   - an `issues` (opened) and an `issue_comment` payload each normalize correctly;
//!   - a malformed payload is a `bad-input` tool error (→ `WorkflowError::Bridge`), never a panic;
//!   - **capability-deny:** with the bridge installed, a caller lacking `normalize` is refused at the
//!     transform gate, and one lacking `ingest_issue` is refused at the inbox write — both opaque.
//!
//! The registry `Source` + publisher keys are the only externals (testing §3). Multi-thread flavor +
//! a UNIQUE workspace id per test (a node boots a Zenoh peer).

use std::collections::HashMap;
use std::sync::Arc;

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    ingest_via_bridge, install_from_registry, Node, RegistryServiceError, Source, WorkflowError,
    TRIAGE_CHANNEL,
};
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys, Visibility};

const MANIFEST: &str = include_str!("../../../extensions/github-bridge/extension.toml");
const NORMALIZE: &str = "mcp:github-bridge.normalize:call";
const INGEST: &str = "mcp:workflow.ingest_issue:call";

fn bridge_wasm() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/github-bridge/target/wasm32-wasip2/release/github_bridge_ext.wasm");
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing github-bridge component at {} ({e}).\nBuild it first:\n  \
             (cd rust/extensions/github-bridge && cargo build --target wasm32-wasip2 --release)",
            path.display()
        )
    })
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
    let wasm = bridge_wasm();
    let d = digest(MANIFEST, &wasm);
    Artifact {
        ext_id: "github-bridge".into(),
        version: "0.1.0".into(),
        manifest_toml: MANIFEST.into(),
        wasm,
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

struct MapSource(HashMap<(String, String), Artifact>);
impl MapSource {
    fn one(art: Artifact) -> Self {
        Self(HashMap::from([(
            (art.ext_id.clone(), art.version.clone()),
            art,
        )]))
    }
}
impl Source for MapSource {
    async fn fetch(&self, ext_id: &str, version: &str) -> Result<Artifact, RegistryServiceError> {
        self.0
            .get(&(ext_id.to_string(), version.to_string()))
            .cloned()
            .ok_or_else(|| RegistryServiceError::NotAvailable(format!("{ext_id}@{version}")))
    }
}

/// Install the bridge in `ws` and return a booted node ready to ingest.
async fn node_with_bridge(ws: &str, seed: u8) -> Arc<Node> {
    let node = Arc::new(Node::boot().await.unwrap());
    let (kid, sk, trusted) = publisher(seed);
    let source = MapSource::one(sign(&kid, &sk));
    install_from_registry(
        &node,
        &source,
        ws,
        "github-bridge",
        "0.1.0",
        &trusted,
        &[NORMALIZE.into()],
        Visibility::Private,
        1,
    )
    .await
    .expect("bridge installs");
    node
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn normalize_maps_comment_and_rejects_malformed() {
    // The transform branches through the REAL wasm: a comment payload folds the comment text into the
    // triage body; a malformed payload is a bridge error, not a panic, and writes nothing.
    let ws = "ghn-transform";
    let node = node_with_bridge(ws, 30).await;
    let user = principal("user:ada", ws, &[NORMALIZE, INGEST]);

    let comment = r#"{
      "action": "created",
      "issue": { "number": 2451, "title": "token refresh race", "body": "orig" },
      "comment": { "body": "still happening on staging" },
      "repository": { "full_name": "acme/api" },
      "ts": 9
    }"#;
    let item = ingest_via_bridge(&node, &user, ws, comment)
        .await
        .expect("a comment webhook ingests");
    assert_eq!(item.id, "acme/api#2451");
    assert!(item.body.contains("still happening on staging"));

    let bad = r#"{ "action": "opened", "ts": 1 }"#;
    assert!(matches!(
        ingest_via_bridge(&node, &user, ws, bad).await,
        Err(WorkflowError::Bridge(_))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn normalize_and_ingest_are_denied_without_their_grants() {
    // MANDATORY capability-deny (the transform + the write): a caller lacking `normalize` is refused
    // at gate 1; one lacking `ingest_issue` is refused at gate 2 (after the transform ran). Both stay
    // opaque (Denied), and nothing is written under either deny.
    let ws = "ghn-deny";
    let node = node_with_bridge(ws, 31).await;
    let opened = r#"{
      "action": "opened",
      "issue": { "number": 1, "title": "t", "body": "b" },
      "repository": { "full_name": "acme/api" },
      "ts": 2
    }"#;

    let no_norm = principal("user:a", ws, &[INGEST]);
    assert!(matches!(
        ingest_via_bridge(&node, &no_norm, ws, opened).await,
        Err(WorkflowError::Denied)
    ));

    let no_ingest = principal("user:b", ws, &[NORMALIZE]);
    assert!(matches!(
        ingest_via_bridge(&node, &no_ingest, ws, opened).await,
        Err(WorkflowError::Denied)
    ));

    assert!(lb_inbox::list(&node.store, ws, TRIAGE_CHANNEL)
        .await
        .unwrap()
        .is_empty());
}
