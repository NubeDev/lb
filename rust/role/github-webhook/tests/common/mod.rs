//! Shared harness for the webhook-receiver integration tests, split out so each test file stays
//! under the 400-line FILE-LAYOUT limit (the same split `github_bridge_test` + `_normalize_test`
//! use). Installs the real `github-bridge` wasm through the registry, mints the ingest principal,
//! builds a signed `POST /webhook` request, and drives the router with `tower::oneshot`.
//!
//! Each test binary that includes this module compiles it independently, so a helper used by only
//! one file looks "unused" to the other — `allow(dead_code)` keeps both builds warning-free.
#![allow(dead_code)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use hmac::{Hmac, Mac};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{install_from_registry, Loaded, Node, RegistryServiceError, Source};
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys, Visibility};
use lb_role_github_webhook::{router, tenant_router, TenantRegistry, WebhookState, WebhookTenant};
use sha2::Sha256;
use tower::ServiceExt; // for `oneshot`

/// The shared HMAC secret the tests sign deliveries with (the bytes a repo's webhook config holds).
pub const SECRET: &[u8] = b"webhook-shared-secret";

pub const NORMALIZE: &str = "mcp:github-bridge.normalize:call";
pub const INGEST: &str = "mcp:workflow.ingest_issue:call";

// --- the real github-bridge component + its manifest -------------------------------------------

const BRIDGE_MANIFEST: &str = include_str!("../../../../extensions/github-bridge/extension.toml");

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

// --- principal + publisher fixtures (mirror github_bridge_test.rs) -----------------------------

/// A workspace-scoped principal holding exactly `caps`.
pub fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
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

/// The grants a caller driving the full ingest holds: the transform tool + the inbox write.
pub fn ingest_caps() -> Vec<&'static str> {
    vec![NORMALIZE, INGEST]
}

fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

fn sign_artifact(
    manifest: &str,
    wasm: Vec<u8>,
    key_id: &str,
    sk: &PublisherSigningKey,
) -> Artifact {
    let d = digest(manifest, &wasm);
    Artifact {
        ext_id: "github-bridge".into(),
        version: "0.1.0".into(),
        manifest_toml: manifest.into(),
        wasm,
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

/// In-memory artifact origin — the only external. Mirrors `github_bridge_test::MapSource`.
struct MapSource {
    artifacts: std::collections::HashMap<(String, String), Artifact>,
}
impl Source for MapSource {
    async fn fetch(&self, ext_id: &str, version: &str) -> Result<Artifact, RegistryServiceError> {
        self.artifacts
            .get(&(ext_id.to_string(), version.to_string()))
            .cloned()
            .ok_or_else(|| RegistryServiceError::NotAvailable(format!("{ext_id}@{version}")))
    }
}

/// Install the real `github-bridge` into `ws` so `ingest_via_bridge` has a tool to call.
pub async fn install_bridge(node: &Node, ws: &str) -> Result<Loaded, RegistryServiceError> {
    let (kid, sk, trusted) = publisher(40);
    let art = sign_artifact(BRIDGE_MANIFEST, bridge_wasm(), &kid, &sk);
    let mut artifacts = std::collections::HashMap::new();
    artifacts.insert((art.ext_id.clone(), art.version.clone()), art);
    let source = MapSource { artifacts };
    install_from_registry(
        node,
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
}

/// Boot a node, install the bridge in `ws`, and build a receiver with the given `caps`.
pub async fn receiver(ws: &str, caps: &[&str]) -> (Arc<Node>, WebhookState) {
    let node = Arc::new(Node::boot().await.unwrap());
    install_bridge(&node, ws).await.unwrap();
    let state =
        WebhookState::from_shared(node.clone(), principal("user:hook", ws, caps), ws, SECRET);
    (node, state)
}

// --- webhook payload + signing -----------------------------------------------------------------

/// A real-shaped `issues` (opened) webhook for `acme/api#2451`. The bridge reads `action`,
/// `issue.number/title/body`, `repository.full_name`, and the injected `ts`.
pub fn issue_opened_webhook(ts: u64) -> String {
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

/// `sha256=<hex>` of `HMAC-SHA256(secret, body)` — the header GitHub sends, computed independently
/// of the crate's verifier so the round-trip is a genuine cross-check.
pub fn signature_for(secret: &[u8], body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
    mac.update(body);
    let mac = mac.finalize().into_bytes();
    let mut hex = String::with_capacity(64);
    for b in mac {
        hex.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        hex.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    format!("sha256={hex}")
}

/// A `POST /webhook` request with the given body and (optional) signature header.
pub fn webhook_req(body: &str, signature: Option<&str>) -> Request<Body> {
    let mut b = Request::builder()
        .method("POST")
        .uri("/webhook")
        .header("content-type", "application/json");
    if let Some(sig) = signature {
        b = b.header("x-hub-signature-256", sig);
    }
    b.body(Body::from(body.to_string())).unwrap()
}

/// A correctly-signed (under [`SECRET`]) `POST /webhook` request.
pub fn signed_req(body: &str) -> Request<Body> {
    let sig = signature_for(SECRET, body.as_bytes());
    webhook_req(body, Some(&sig))
}

/// Drive the router and return the response status (the inbox is the side-effect under test).
pub async fn status(state: WebhookState, req: Request<Body>) -> StatusCode {
    router(state).oneshot(req).await.unwrap().status()
}

// --- multi-tenant front-door helpers -----------------------------------------------------------

/// Build a `WebhookTenant` for `ws` with the given `caps` and `secret` (its own webhook key).
pub fn tenant(ws: &str, caps: &[&str], secret: &[u8]) -> WebhookTenant {
    WebhookTenant::new(principal("user:hook", ws, caps), ws, secret.to_vec())
}

/// A `POST /webhook/{slug}` request with the given body and (optional) signature header.
pub fn tenant_req(slug: &str, body: &str, signature: Option<&str>) -> Request<Body> {
    let mut b = Request::builder()
        .method("POST")
        .uri(format!("/webhook/{slug}"))
        .header("content-type", "application/json");
    if let Some(sig) = signature {
        b = b.header("x-hub-signature-256", sig);
    }
    b.body(Body::from(body.to_string())).unwrap()
}

/// A `POST /webhook/{slug}` request correctly signed under `secret`.
pub fn signed_tenant_req(slug: &str, body: &str, secret: &[u8]) -> Request<Body> {
    let sig = signature_for(secret, body.as_bytes());
    tenant_req(slug, body, Some(&sig))
}

/// Drive the multi-tenant router and return the response status.
pub async fn tenant_status(registry: TenantRegistry, req: Request<Body>) -> StatusCode {
    tenant_router(registry).oneshot(req).await.unwrap().status()
}
