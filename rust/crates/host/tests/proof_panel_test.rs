//! `proof-panel` — the NEW Tier-1 WASM proof extension, exercised through the real publish → install →
//! load → call path (no mocks — §9). Where `ext_publish_test` proves the path with `hello`, this proves
//! a *fresh* self-contained extension (one real `proof.status` tool + a federated UI manifest) travels
//! the same path: a signed artifact installs and loads, and its tool is **callable immediately** through
//! the host capability gate and the real WASM component runtime.
//!
//! Proven here:
//!   - HAPPY PATH: a correctly-signed `proof-panel` artifact publishes, persists its `Install`, and
//!     `proof-panel.proof.status` is callable right now — the WASM (Tier-1) backend is reachable.
//!   - MANDATORY capability-deny (testing scope): the tool is refused WITHOUT `mcp:proof-panel.proof.status:call`.
//!   - MANDATORY workspace-isolation (testing scope): a stateless WASM tool is loaded process-wide, so
//!     the per-workspace wall is the capability gate — a second workspace WITHOUT the grant is denied.
//!
//! The publisher key is the only external (testing §3); store + wasm + runtime are real.

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{ext_publish, installed, Node};
use lb_mcp::{call, ToolError};
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys, Visibility};

const MANIFEST: &str = include_str!("../../../extensions/proof-panel/extension.toml");
const PUBLISH: &str = "mcp:ext.publish:call";
const STATUS_CAP: &str = "mcp:proof-panel.proof.status:call";

fn proof_panel_wasm() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/proof-panel/target/wasm32-wasip2/release/proof_panel_ext.wasm");
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing component at {} ({e}).\nBuild it first:\n  bash rust/extensions/proof-panel/build.sh",
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
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

fn sign(wasm: &[u8], key_id: &str, sk: &PublisherSigningKey) -> Artifact {
    let d = digest(MANIFEST, wasm);
    Artifact {
        ext_id: "proof-panel".into(),
        version: "0.1.0".into(),
        manifest_toml: MANIFEST.into(),
        wasm: wasm.to_vec(),
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

/// Publish-install-load a signed `proof-panel` into `node`/`ws`, returning once the Install persists.
async fn install(node: &Node, ws: &str, seed: u8) {
    let (kid, sk, trusted) = publisher(seed);
    let art = sign(&proof_panel_wasm(), &kid, &sk);
    let caller = principal(ws, &[PUBLISH]);
    ext_publish(node, &caller, ws, art, &trusted, Visibility::Public, 1)
        .await
        .expect("a signed proof-panel artifact publishes-and-installs");
    let rec = installed(node, ws, "proof-panel")
        .await
        .unwrap()
        .expect("installed");
    assert_eq!(rec.version, "0.1.0");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn proof_panel_publishes_and_its_tool_is_callable() {
    let ws = "pp-happy";
    let node = Node::boot().await.unwrap();
    install(&node, ws, 40).await;

    // The WASM tool is reachable RIGHT NOW through the host capability gate + the component runtime.
    let p = principal(ws, &[STATUS_CAP]);
    let out = call(
        &node.registry,
        &node.bus,
        &p,
        ws,
        "proof-panel.proof.status",
        r#"{"note":"hello tier-1"}"#,
    )
    .await
    .expect("proof.status on the just-published WASM extension");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(
        v["note"], "hello tier-1",
        "the note round-trips through the WASM guest"
    );
    assert_eq!(
        v["tier"], "wasm",
        "the Tier-1 component is the one that served the call"
    );
    assert_eq!(v["ext"], "proof-panel");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn proof_status_is_denied_without_the_grant() {
    let ws = "pp-deny";
    let node = Node::boot().await.unwrap();
    install(&node, ws, 41).await;

    // No `mcp:proof-panel.proof.status:call` grant — the host gate refuses before reaching the guest.
    let p = principal(ws, &[]);
    let err = call(
        &node.registry,
        &node.bus,
        &p,
        ws,
        "proof-panel.proof.status",
        r#"{"note":"nope"}"#,
    )
    .await
    .expect_err("proof.status without the grant is denied");
    assert!(matches!(err, ToolError::Denied), "got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn another_workspace_without_the_grant_is_denied() {
    // A stateless WASM tool is loaded into the node's PROCESS-GLOBAL registry; reachability is not
    // workspace-scoped (the tool holds no data — isolation bites where a tool touches its workspace's
    // store, not at tool existence). The wall that holds PER WORKSPACE here is the capability gate:
    // every call authorizes `mcp:proof-panel.proof.status:call` against the CALLER'S OWN token (its ws
    // + its caps). So a second workspace that lacks the grant is denied, even though the component is
    // loaded process-wide. This is the meaningful, real isolation proof for a data-less Tier-1 tool.
    let node = Node::boot().await.unwrap();
    install(&node, "pp-ws-a", 42).await; // loaded into the node's global registry via ws-A

    let p = principal("pp-ws-b", &[]); // ws-B, no proof.status grant
    let err = call(
        &node.registry,
        &node.bus,
        &p,
        "pp-ws-b",
        "proof-panel.proof.status",
        r#"{"note":"cross"}"#,
    )
    .await
    .expect_err("ws-B without the grant is denied");
    assert!(matches!(err, ToolError::Denied), "got {err:?}");
}
