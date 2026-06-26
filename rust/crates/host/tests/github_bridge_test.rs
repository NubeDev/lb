//! S7 (github-bridge slice) — the `github-bridge` packaged as an installed Tier-1 wasm artifact
//! (github-bridge scope; the S6 `github-bridge` deferral resolved). It proves the full
//! installed-artifact lifecycle on a SECOND real extension (the first was `hello`), and the
//! pure-transform composition: the sandboxed `github-bridge.normalize` tool maps a raw GitHub
//! webhook to `{issue_id, payload, ts}`, and the HOST (`ingest_via_bridge`) writes the canonical
//! issue to the `triage` inbox — the guest never calls back (the WIT world imports only `log`).
//!
//! This file covers the install-lifecycle categories; the transform branches + per-tool deny live in
//! `github_bridge_normalize_test.rs`. Mandatory categories (testing §2) at this slice's surfaces:
//!   - **capability-deny:** install refused without `mcp:registry.install:call`;
//!   - **workspace-isolation:** ws-B has no install record (store wall) and its ingest writes only
//!     ws-B's namespace — the node-global stateless instance is shared, the data is not;
//!   - **offline:** once cached, the bridge installs with the registry `Source` offline;
//!   - **rollback:** install 0.2.0 then 0.1.0 — an inbox `Item` ingested before the swap survives.
//!
//! The registry `Source` + publisher keys are the only externals (testing §3); store + bus + the
//! real `github_bridge_ext.wasm` are real. Multi-thread flavor + a UNIQUE workspace id per test
//! (a node boots a Zenoh peer).

use std::collections::HashMap;
use std::sync::Arc;

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    ingest_via_bridge, install_from_registry, installed, Node, RegistryServiceError, Source,
    WorkflowError, TRIAGE_CHANNEL,
};
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys, Visibility};

// --- the real component + its manifest ---------------------------------------------------------

const MANIFEST_V1: &str = include_str!("../../../extensions/github-bridge/extension.toml");

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

/// A v0.2.0 manifest: the same wasm, the `version` field bumped. A distinct, valid artifact (the
/// digest binds manifest+wasm), so installing it then re-installing v0.1.0 is the rollback path.
fn manifest_v2() -> String {
    MANIFEST_V1.replace("version     = \"0.1.0\"", "version     = \"0.2.0\"")
}

// --- principal + publisher fixtures ------------------------------------------------------------

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

const INSTALL: &str = "mcp:registry.install:call";
const NORMALIZE: &str = "mcp:github-bridge.normalize:call";
const INGEST: &str = "mcp:workflow.ingest_issue:call";

/// The grants a caller driving `ingest_via_bridge` holds end to end: the transform tool + the inbox
/// write. (Install is a separate, admin-time grant.)
fn ingest_caps() -> Vec<&'static str> {
    vec![NORMALIZE, INGEST]
}

fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

fn sign(
    version: &str,
    manifest: &str,
    wasm: Vec<u8>,
    key_id: &str,
    sk: &PublisherSigningKey,
) -> Artifact {
    let d = digest(manifest, &wasm);
    Artifact {
        ext_id: "github-bridge".into(),
        version: version.into(),
        manifest_toml: manifest.into(),
        wasm,
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

// --- the test Source (the only external) -------------------------------------------------------

/// In-memory artifact origin keyed by `(ext_id, version)`, switchable offline (every fetch errors,
/// counted) so the cached path can be proven to never touch it.
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
    fn fetches(&self) -> usize {
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

// --- a representative GitHub webhook payload ---------------------------------------------------

/// A real-shaped `issues` (opened) webhook. The bridge reads `action`, `issue.number/title/body`,
/// `repository.full_name`, and the injected `ts`; unknown fields are ignored.
fn issue_opened_webhook(ts: u64) -> String {
    format!(
        r#"{{
          "action": "opened",
          "issue": {{
            "number": 2451,
            "title": "token refresh race",
            "body": "two refreshes race under load",
            "user": {{ "login": "ada" }}
          }},
          "repository": {{ "full_name": "acme/api" }},
          "ts": {ts}
        }}"#
    )
}

async fn install_bridge(
    node: &Node,
    source: &MapSource,
    ws: &str,
    version: &str,
    trusted: &TrustedKeys,
    ts: u64,
) -> Result<lb_host::Loaded, RegistryServiceError> {
    // `request = []` in the manifest, so the admin-approved set for the bridge tool is just its
    // call grant — granted = requested ∩ approved is computed host-side.
    install_from_registry(
        node,
        source,
        ws,
        "github-bridge",
        version,
        trusted,
        &[NORMALIZE.into()],
        Visibility::Private,
        ts,
    )
    .await
}

// === tests =====================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn installs_the_bridge_and_ingests_a_webhook_end_to_end() {
    // HAPPY / ROUND-TRIP: a signed github-bridge installs through the registry; a raw GitHub webhook
    // run through normalize → ingest_issue lands ONE canonical triage item (idempotent on retry).
    let ws = "gh-happy";
    let node = Arc::new(Node::boot().await.unwrap());
    let (kid, sk, trusted) = publisher(20);
    let art = sign("0.1.0", MANIFEST_V1, bridge_wasm(), &kid, &sk);
    let source = MapSource::new(vec![art]);

    let loaded = install_bridge(&node, &source, ws, "0.1.0", &trusted, 1)
        .await
        .expect("signed bridge installs");
    assert!(loaded.tools.contains(&"normalize".to_string()));

    let user = principal("user:ada", ws, &ingest_caps());
    let item = ingest_via_bridge(&node, &user, ws, &issue_opened_webhook(7))
        .await
        .expect("webhook ingests");

    // The host wrote the canonical issue to triage: id scoped by repo, body normalized, S6 tags.
    assert_eq!(item.channel, TRIAGE_CHANNEL);
    assert_eq!(item.id, "acme/api#2451");
    assert!(item.body.contains("needs:triage"));
    assert!(item.body.contains("token refresh race"));

    // Idempotent on the normalized issue_id: a re-delivered webhook upserts the SAME one item.
    ingest_via_bridge(&node, &user, ws, &issue_opened_webhook(8))
        .await
        .unwrap();
    let items = lb_inbox::list(&node.store, ws, TRIAGE_CHANNEL)
        .await
        .unwrap();
    assert_eq!(
        items.len(),
        1,
        "a retried webhook produces exactly one item"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn install_is_denied_without_the_grant() {
    // MANDATORY capability-deny (install): the registry gate refuses install without
    // mcp:registry.install:call — asserted at the host-side chokepoint, transport-independent.
    let ws = "gh-deny-install";
    let p_nogrant = principal("user:mallory", ws, &[]);
    assert!(matches!(
        lb_host::authorize_registry(&p_nogrant, ws, "install").unwrap_err(),
        RegistryServiceError::Denied
    ));
    let p_ok = principal("user:admin", ws, &[INSTALL]);
    assert!(lb_host::authorize_registry(&p_ok, ws, "install").is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_ingest_lands_in_ws_b_never_ws_a() {
    // MANDATORY workspace-isolation across store + MCP: install the bridge in ws-A only. ws-B has no
    // install record (store wall); the loaded wasm instance is node-global + STATELESS, so a granted
    // ws-B caller may run it — but every effect lands in ws-B's namespace, never ws-A's (the data
    // wall is the hard one; see debugging/extensions/loaded-extension-instance-is-node-global.md).
    let ws_a = "gh-iso-a";
    let ws_b = "gh-iso-b";
    let node = Arc::new(Node::boot().await.unwrap());
    let (kid, sk, trusted) = publisher(22);
    let art = sign("0.1.0", MANIFEST_V1, bridge_wasm(), &kid, &sk);
    let source = MapSource::new(vec![art]);
    install_bridge(&node, &source, ws_a, "0.1.0", &trusted, 1)
        .await
        .unwrap();

    // Store isolation: the install record exists in ws-A, is absent in ws-B.
    assert!(installed(&node, ws_a, "github-bridge")
        .await
        .unwrap()
        .is_some());
    assert!(installed(&node, ws_b, "github-bridge")
        .await
        .unwrap()
        .is_none());

    // MCP/data isolation: a fully-granted ws-B caller CAN run the (stateless, node-global) bridge
    // instance — but every effect it has lands in ws-B's namespace, NEVER ws-A's. The hard wall is
    // the data, not the shared pure-transform instance (stateless extensions, §3.4): an ingest by
    // ws-B writes ws-B's inbox and leaves ws-A's untouched.
    let b_user = principal("user:b", ws_b, &ingest_caps());
    ingest_via_bridge(&node, &b_user, ws_b, &issue_opened_webhook(2))
        .await
        .expect("ws-B runs the shared stateless bridge, writing into ITS OWN namespace");

    let a_items = lb_inbox::list(&node.store, ws_a, TRIAGE_CHANNEL)
        .await
        .unwrap();
    let b_items = lb_inbox::list(&node.store, ws_b, TRIAGE_CHANNEL)
        .await
        .unwrap();
    assert!(
        a_items.is_empty(),
        "ws-A's inbox is untouched by ws-B's ingest"
    );
    assert_eq!(
        b_items.len(),
        1,
        "ws-B's ingest landed in ws-B's own namespace"
    );

    // And the capability wall still bites: a ws-B caller WITHOUT the grants is denied (gate 1).
    let b_nogrant = principal("user:c", ws_b, &[]);
    assert!(matches!(
        ingest_via_bridge(&node, &b_nogrant, ws_b, &issue_opened_webhook(3)).await,
        Err(WorkflowError::Denied)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cached_bridge_installs_with_the_registry_offline() {
    // MANDATORY offline: pull once online to cache, flip the Source offline, install again. It must
    // succeed AND not have fetched — the cached, already-verified bytes are served locally.
    let ws = "gh-offline";
    let node = Arc::new(Node::boot().await.unwrap());
    let (kid, sk, trusted) = publisher(23);
    let art = sign("0.1.0", MANIFEST_V1, bridge_wasm(), &kid, &sk);
    let source = MapSource::new(vec![art]);

    install_bridge(&node, &source, ws, "0.1.0", &trusted, 1)
        .await
        .unwrap();
    let after_first = source.fetches();
    assert_eq!(after_first, 1, "the first install fetched once");

    source.go_offline();
    install_bridge(&node, &source, ws, "0.1.0", &trusted, 2)
        .await
        .expect("the cached artifact installs offline");
    assert_eq!(
        source.fetches(),
        after_first,
        "the cached path never touched the offline source"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rollback_keeps_the_ingested_inbox_intact() {
    // MANDATORY rollback/hot-reload: install 0.2.0, ingest a webhook (durable STATE), roll back to
    // 0.1.0 (install the prior version) — the inbox item survives (no durable guest state, §3.4).
    let ws = "gh-rollback";
    let node = Arc::new(Node::boot().await.unwrap());
    let (kid, sk, trusted) = publisher(24);
    let v2 = sign("0.2.0", &manifest_v2(), bridge_wasm(), &kid, &sk);
    let v1 = sign("0.1.0", MANIFEST_V1, bridge_wasm(), &kid, &sk);
    let source = MapSource::new(vec![v1, v2]);

    // Install 0.2.0, ingest an issue.
    install_bridge(&node, &source, ws, "0.2.0", &trusted, 1)
        .await
        .unwrap();
    assert_eq!(
        installed(&node, ws, "github-bridge")
            .await
            .unwrap()
            .unwrap()
            .version,
        "0.2.0"
    );
    let user = principal("user:ada", ws, &ingest_caps());
    ingest_via_bridge(&node, &user, ws, &issue_opened_webhook(2))
        .await
        .unwrap();

    // Roll back = install the prior version (the same verb). The Install record is now 0.1.0.
    install_bridge(&node, &source, ws, "0.1.0", &trusted, 3)
        .await
        .expect("rollback installs the prior version");
    assert_eq!(
        installed(&node, ws, "github-bridge")
            .await
            .unwrap()
            .unwrap()
            .version,
        "0.1.0"
    );

    // The durable inbox state written before the swap is INTACT after the rollback.
    let items = lb_inbox::list(&node.store, ws, TRIAGE_CHANNEL)
        .await
        .unwrap();
    assert_eq!(items.len(), 1, "the ingested issue survived the rollback");
    assert_eq!(items[0].id, "acme/api#2451");
}
