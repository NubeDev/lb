//! The registry's HTTP transport, end to end — the `registry-host` server + `HttpSource`, replacing
//! the in-memory `MapSource` stub. Proves the verify-before-cache / offline / isolation / deny
//! guarantees hold over a REAL socket exactly as they did in-memory.
//!
//! The server is served on an ephemeral bound port (the transport IS the thing under test, so a real
//! socket, not a `tower::oneshot`). The `Source` and the publisher keys are the only externals
//! (testing §3); store + wasm are real. Each test boots a Node (→ a Zenoh peer) → multi-thread
//! flavor + a unique workspace id.

use std::net::SocketAddr;

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{install_from_registry, installed, pull, Node, RegistryServiceError};
use lb_mcp::call;
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys, Visibility};
use lb_role_registry_host::{router, ArtifactStore, HttpSource};

// --- manifest + wasm (the real `hello` component) ---------------------------------------------

const MANIFEST_V1: &str = include_str!("../../../extensions/hello/extension.toml");

fn hello_v1() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing component at {} ({e}).\nBuild it first:\n  \
             (cd rust/extensions/hello && cargo build --target wasm32-wasip2 --release)",
            path.display()
        )
    })
}

// --- principal + publisher fixtures (mirrors registry_test.rs) --------------------------------

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

const PULL: &str = "mcp:registry.pull:call";
const INSTALL: &str = "mcp:registry.install:call";

/// A deterministic publisher (key from a fixed seed) + the matching `TrustedKeys` a workspace holds.
fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

/// Sign `(manifest, wasm)` as `key_id` — a correctly-signed artifact.
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

/// Serve `store` on an ephemeral port and return the base URL an `HttpSource` points at. The serve
/// task is detached; dropping the test ends the process and reaps it.
async fn serve(store: ArtifactStore) -> (String, SocketAddr) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(store);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), addr)
}

// === tests ====================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn installs_a_signed_artifact_pulled_over_http() {
    // THE HAPPY PATH over a real socket: the registry-host serves a signed artifact, HttpSource pulls
    // it, pull verifies + caches it, install loads the real wasm, and its tool runs.
    let ws = "http-happy";
    let node = Node::boot().await.unwrap();
    let (kid, sk, trusted) = publisher(20);
    let art = sign("hello", "0.1.0", MANIFEST_V1, &hello_v1(), &kid, &sk);

    let (base, _addr) = serve(ArtifactStore::new(vec![art])).await;
    let source = HttpSource::new(&base);

    let loaded = install_from_registry(
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
    .expect("signed artifact installs over HTTP");
    assert!(loaded.tools.contains(&"echo".to_string()));

    // The Install record persisted and the tool is callable (subject to its own grant).
    assert_eq!(
        installed(&node, ws, "hello")
            .await
            .unwrap()
            .unwrap()
            .version,
        "0.1.0"
    );
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
async fn cached_artifact_installs_with_the_server_offline() {
    // OFFLINE/SYNC category over HTTP: pull once (cache it), then flip the origin offline and install
    // AGAIN — the cached path short-circuits, HttpSource::fetch is never called, the install succeeds.
    let ws = "http-offline";
    let node = Node::boot().await.unwrap();
    let (kid, sk, trusted) = publisher(21);
    let art = sign("hello", "0.1.0", MANIFEST_V1, &hello_v1(), &kid, &sk);

    let store = ArtifactStore::new(vec![art]);
    let (base, _addr) = serve(store.clone()).await;
    let source = HttpSource::new(&base);

    // First install: cache miss → fetch over HTTP → verify → cache.
    install_from_registry(
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
    .expect("first install caches the artifact");

    // The origin goes dark — any fetch now 404s (an unreachable server looks the same).
    store.set_offline(true);

    // Second install of the SAME version succeeds entirely from cache (no fetch reaches the origin).
    let loaded = install_from_registry(
        &node,
        &source,
        ws,
        "hello",
        "0.1.0",
        &trusted,
        &["mcp:hello.echo:call".into()],
        Visibility::Private,
        2,
    )
    .await
    .expect("cached artifact installs with the server offline");
    assert!(loaded.tools.contains(&"echo".to_string()));

    // Proof the offline path is real: a NEVER-cached version DOES fail now (the origin is dark).
    let err = pull(
        &node.store,
        &source,
        ws,
        "hello",
        "9.9.9",
        &trusted,
        Visibility::Private,
        3,
    )
    .await
    .expect_err("an uncached version cannot be pulled while offline");
    assert!(matches!(err, RegistryServiceError::NotAvailable(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_artifact_tampered_in_transit_is_rejected() {
    // SIGNING/VERIFICATION over the wire: the origin serves bytes whose digest no longer matches the
    // signature (a tamper in transit). pull's verify-before-cache rejects it — nothing is cached,
    // proving trust is re-established CLIENT-side, never inherited from the transport.
    let ws = "http-tamper";
    let node = Node::boot().await.unwrap();
    let (kid, sk, trusted) = publisher(22);
    let mut art = sign("hello", "0.1.0", MANIFEST_V1, &hello_v1(), &kid, &sk);
    // Tamper AFTER signing — the served bytes no longer match the signed digest.
    art.wasm.extend_from_slice(b"\x00mitm");

    let (base, _addr) = serve(ArtifactStore::new(vec![art])).await;
    let source = HttpSource::new(&base);

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
    .expect_err("a tampered-in-transit artifact is refused");
    assert!(matches!(err, RegistryServiceError::Unverified));

    // Nothing cached: a re-pull still cannot verify it (the tamper wasn't laundered into the cache).
    assert!(installed(&node, ws, "hello").await.unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_see_ws_a_artifact_pulled_over_the_same_server() {
    // WORKSPACE-ISOLATION over a SHARED server: ws-A pulls + caches an artifact; ws-B, pulling the
    // same version from the SAME origin but trusting a DIFFERENT key, is refused — and ws-A's cache /
    // catalog entry is never visible to ws-B. The server is shared; the cache + trust are not.
    let ws_a = "http-iso-a";
    let ws_b = "http-iso-b";
    let node = Node::boot().await.unwrap();

    // ws-A trusts key 23 (which signed the artifact); ws-B trusts key 24 (which did not).
    let (kid, sk, trust_a) = publisher(23);
    let (_other_id, _other_sk, trust_b) = publisher(24);
    let art = sign("hello", "0.1.0", MANIFEST_V1, &hello_v1(), &kid, &sk);

    let (base, _addr) = serve(ArtifactStore::new(vec![art])).await;
    let source = HttpSource::new(&base);

    // ws-A installs (its trusted key signed it) → cached + catalogued in ws-A's namespace.
    install_from_registry(
        &node,
        &source,
        ws_a,
        "hello",
        "0.1.0",
        &trust_a,
        &["mcp:hello.echo:call".into()],
        Visibility::Private,
        1,
    )
    .await
    .expect("ws-A installs its trusted artifact");

    // ws-B pulls the same bytes from the same server but trusts a foreign key → Unverified. ws-A's
    // cache does NOT help ws-B: the cache is workspace-namespaced, so ws-B takes the fetch+verify
    // path and is rejected at verification.
    let err = pull(
        &node.store,
        &source,
        ws_b,
        "hello",
        "0.1.0",
        &trust_b,
        Visibility::Private,
        2,
    )
    .await
    .expect_err("ws-B cannot verify ws-A's artifact under its own trust set");
    assert!(matches!(err, RegistryServiceError::Unverified));

    // And ws-B has nothing installed — ws-A's install never leaked across the wall.
    assert!(installed(&node, ws_b, "hello").await.unwrap().is_none());
    // ws-A still has its install (the empty ws-B view is isolation, not a failed write).
    assert_eq!(
        installed(&node, ws_a, "hello")
            .await
            .unwrap()
            .unwrap()
            .version,
        "0.1.0"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn install_over_http_is_denied_without_the_grant() {
    // MANDATORY capability-deny: the gate is host-side and TRANSPORT-INDEPENDENT — swapping the
    // in-memory source for HttpSource changes nothing. A caller without the grant is refused at the
    // same `authorize_registry` chokepoint, before any fetch.
    let ws = "http-deny";
    let p_nogrant = principal(ws, &[]);
    assert!(matches!(
        lb_host::authorize_registry(&p_nogrant, ws, "pull").unwrap_err(),
        RegistryServiceError::Denied
    ));
    let p_other = principal(ws, &["mcp:registry.list:call"]);
    assert!(matches!(
        lb_host::authorize_registry(&p_other, ws, "install").unwrap_err(),
        RegistryServiceError::Denied
    ));
    // With the grant, the gate passes (the deny is the grant's doing, not a blanket no).
    let p_ok = principal(ws, &[PULL, INSTALL]);
    assert!(lb_host::authorize_registry(&p_ok, ws, "pull").is_ok());
    assert!(lb_host::authorize_registry(&p_ok, ws, "install").is_ok());
}
