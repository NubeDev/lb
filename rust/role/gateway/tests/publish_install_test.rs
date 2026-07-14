//! The **publish → install → load → callable** flow over the gateway (lifecycle-management scope:
//! the gap where `POST /extensions` stopped at the catalog and nothing brought the component online).
//! Proves end to end, through the REAL routes + the REAL WASM component (no mocks, CLAUDE.md rule #9):
//!   1. a signed artifact for the real `hello-v2` wasm, published with its publisher key **trusted**,
//!      returns `204` AND the extension is then **callable** (`hello.echo` → v2 output) — i.e. publish
//!      actually loaded it into the runtime, not merely cataloged it;
//!   2. the same artifact with an **untrusted** publisher key is `422` and **nothing is reachable**
//!      (verify-before-store: the capability gate and the signature gate are independent);
//!   3. a caller **without** `mcp:ext.publish:call` is denied `403` server-side (the boundary is the
//!      server, not the UI cap-gate).
//!
//! This is exactly what `lb-pack` produces for the dev flow — the test signs inline with the SAME
//! `digest` + Ed25519 idiom the packager and the node verify with, so a packaged artifact verifies by
//! construction.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::{bearer, gateway, json_post, json_post_sized, token};
use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys};
use lb_role_gateway::{router, Gateway};
use serde_json::{json, Value};
use tower::ServiceExt;

const MANIFEST: &str = include_str!("../../../extensions/hello-v2/extension.toml");
const WASM: &[u8] =
    include_bytes!("../../../extensions/hello-v2/target/wasm32-wasip2/release/hello_v2_ext.wasm");

const PUBLISH_CAP: &str = "mcp:ext.publish:call";
const ECHO_CAP: &str = "mcp:hello.echo:call";
const LIST_CAP: &str = "mcp:ext.list:call";

/// A dev publisher (deterministic seed) + the `TrustedKeys` allow-list that trusts it — exactly what
/// `lb-pack` writes and what `LB_TRUSTED_PUBKEYS` seeds at the gateway.
fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

/// Sign the real hello-v2 artifact — the packager's job, inline.
fn artifact(key_id: &str, sk: &PublisherSigningKey) -> Artifact {
    let d = digest(MANIFEST, WASM);
    Artifact {
        ext_id: "hello".into(),
        version: "0.2.0".into(),
        manifest_toml: MANIFEST.into(),
        wasm: WASM.to_vec(),
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publish_installs_and_loads_a_trusted_artifact_so_it_is_callable() {
    let (gw, key) = gateway().await;
    let (id, sk, trusted) = publisher(7);
    // The gateway trusts this publisher (what `LB_TRUSTED_PUBKEYS` does in dev).
    let gw = Gateway::new(Arc::clone(&gw.node), key.clone(), common::NOW).with_trusted(trusted);

    let tok = token(
        &key,
        "user:admin",
        "acme",
        &[PUBLISH_CAP, ECHO_CAP, LIST_CAP],
    );

    // Publish → 204. This must also INSTALL + LOAD the component, not just catalog it.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/extensions",
                serde_json::to_value(artifact(&id, &sk)).unwrap(),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "trusted publish installs"
    );

    // It now appears in the install list.
    let resp = router(gw.clone())
        .oneshot(bearer(common::get_req("/extensions"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let rows: Value = common::json_body(resp).await;
    assert!(
        rows.as_array()
            .unwrap()
            .iter()
            .any(|r| r["ext"] == "hello" || r["ext_id"] == "hello"),
        "the published extension is listed: {rows}"
    );

    // And — the load-bearing assertion — its tool is CALLABLE through the runtime. v2 output carries `v:2`.
    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/mcp/call",
                json!({ "tool": "hello.echo", "args": { "msg": "hi" } }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "the loaded tool is callable");
    let out: Value = common::json_body(resp).await;
    assert_eq!(out["v"], 2, "the v2 component is what got loaded: {out}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_untrusted_publisher_is_422_and_nothing_is_installed() {
    let (gw, key) = gateway().await;
    // The artifact is signed by pub-9, but the gateway trusts only pub-7 → verification fails.
    let (_trusted_id, _trusted_sk, trusted) = publisher(7);
    let (foreign_id, foreign_sk, _) = publisher(9);
    let gw = Gateway::new(Arc::clone(&gw.node), key.clone(), common::NOW).with_trusted(trusted);
    let tok = token(&key, "user:admin", "acme", &[PUBLISH_CAP, ECHO_CAP]);

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/extensions",
                serde_json::to_value(artifact(&foreign_id, &foreign_sk)).unwrap(),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "untrusted publish is 422"
    );

    // Nothing was installed → the tool is not callable (404/forbidden, never an echo).
    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/mcp/call",
                json!({ "tool": "hello.echo", "args": { "msg": "hi" } }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "a rejected artifact left nothing to call"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publish_without_the_capability_is_denied_server_side() {
    let (gw, key) = gateway().await;
    let (id, sk, trusted) = publisher(7);
    let gw = Gateway::new(Arc::clone(&gw.node), key.clone(), common::NOW).with_trusted(trusted);
    // A valid session, fully trusted artifact, but NO ext.publish cap — the server must refuse.
    let tok = token(&key, "user:mallory", "acme", &["bus:chan/*:pub"]);

    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/extensions",
                serde_json::to_value(artifact(&id, &sk)).unwrap(),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "no cap → 403 server-side"
    );
}

/// Regression (native-tier publish): a NATIVE artifact packs a host-target BINARY (megabytes) into the
/// signed `wasm` field, JSON-encoded as a byte array — several MiB on the wire, far past axum's 2 MiB
/// default request-body limit. WASM artifacts (tens–hundreds of KiB) never hit it, so a native ext
/// publishing over `POST /extensions` used to 413 ("length limit exceeded") before the body was ever
/// read. The route now raises its body limit; this asserts a >2 MiB body is BUFFERED (not 413) and
/// reaches verification. The oversized payload is not a trusted artifact, so it lands at 422 (verify),
/// which is exactly the point: it got PAST the body limit to be judged on its merits.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publish_accepts_a_body_larger_than_the_2mib_default_native_binary_size() {
    let (gw, key) = gateway().await;
    let (id, sk, trusted) = publisher(7);
    let gw = Gateway::new(Arc::clone(&gw.node), key.clone(), common::NOW).with_trusted(trusted);
    let tok = token(&key, "user:admin", "acme", &[PUBLISH_CAP]);

    // A 3 MiB payload in the `wasm` field — comfortably past the old 2 MiB default, the size class of a
    // real native sidecar binary. Signed with a trusted key over its real digest so ONLY the body limit,
    // not the signature or the cap, could reject it. (The bytes are not a valid component; if it ever
    // gets past verify it would fail to load — but it is a native-tier size test, and the manifest here
    // is the wasm hello manifest, so it verifies-then-fails-load rather than 413.)
    let big = vec![0u8; 3 * 1024 * 1024];
    let d = digest(MANIFEST, &big);
    let big_artifact = Artifact {
        ext_id: "hello".into(),
        version: "0.2.0".into(),
        manifest_toml: MANIFEST.into(),
        wasm: big,
        digest_hex: digest_hex(&d),
        publisher_key_id: id,
        signature: sk.sign(&d).to_bytes().to_vec(),
    };

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/extensions", serde_json::to_value(big_artifact).unwrap()),
            &tok,
        ))
        .await
        .unwrap();

    // The load-bearing assertion: NOT 413. The body was buffered past the old 2 MiB limit and judged.
    assert_ne!(
        resp.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "a native-sized (>2 MiB) publish body must be accepted, not 413"
    );
}

/// Build a padded-but-**trusted** artifact of ~`wasm_bytes` MiB in the `wasm` field (a real digest +
/// signature over its bytes, so ONLY the body limit — never the signature or the cap — can reject it).
/// The bytes are not a valid component, so a body that clears the limit lands past verify at load; the
/// point of these tests is purely WHERE the body-size boundary sits.
fn padded_artifact(wasm_bytes: usize, id: &str, sk: &PublisherSigningKey) -> Artifact {
    let big = vec![0u8; wasm_bytes];
    let d = digest(MANIFEST, &big);
    Artifact {
        ext_id: "hello".into(),
        version: "0.2.0".into(),
        manifest_toml: MANIFEST.into(),
        wasm: big,
        digest_hex: digest_hex(&d),
        publisher_key_id: id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

/// A publish body just OVER the configured upload ceiling must 413 — with a DESCRIPTIVE message (the
/// size AND the limit), not a bare "length limit exceeded". We pin a small 1 MiB ceiling via
/// `with_max_extension_upload_bytes` so the test is fast (no 384 MiB allocation) and deterministic; the
/// mechanism is identical at the production default. A ~2 MiB `wasm` field lands the encoded body a bit
/// over 1 MiB → the handler's descriptive reject fires (still under the layer's `ceiling + margin` cap,
/// so it reaches the handler rather than the layer's bare 413).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_body_over_the_configured_limit_is_413_with_a_descriptive_error() {
    let (gw, key) = gateway().await;
    let (id, sk, trusted) = publisher(7);
    let gw = Gateway::new(Arc::clone(&gw.node), key.clone(), common::NOW)
        .with_trusted(trusted)
        .with_max_extension_upload_bytes(1024 * 1024); // 1 MiB ceiling
    let tok = token(&key, "user:admin", "acme", &[PUBLISH_CAP]);

    // ~2 MiB of wasm → an encoded body comfortably over the 1 MiB ceiling but under ceiling+16 MiB
    // margin, so the request reaches the handler and gets the descriptive 413 (not the layer's).
    let resp = router(gw)
        .oneshot(bearer(
            json_post_sized(
                "/extensions",
                serde_json::to_value(padded_artifact(2 * 1024 * 1024, &id, &sk)).unwrap(),
            ),
            &tok,
        ))
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "a body over the configured limit is 413"
    );
    let msg = common::body_text(resp).await;
    assert!(
        msg.contains("exceeds the upload limit"),
        "the 413 body must describe the size vs limit, got: {msg:?}"
    );
    assert!(
        msg.contains("MiB"),
        "the descriptive error reports human-readable sizes, got: {msg:?}"
    );
}

/// The companion to the reject test: a body UNDER the configured ceiling clears the size gate and
/// reaches the real verify/install path (a padded body is not a valid component, so it verifies-then-
/// fails-load rather than 204 — but it is NOT rejected for size). Pinning the SAME 1 MiB ceiling, a
/// tiny artifact must NOT 413.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_body_under_the_configured_limit_clears_the_size_gate() {
    let (gw, key) = gateway().await;
    let (id, sk, trusted) = publisher(7);
    let gw = Gateway::new(Arc::clone(&gw.node), key.clone(), common::NOW)
        .with_trusted(trusted)
        .with_max_extension_upload_bytes(1024 * 1024); // 1 MiB ceiling
    let tok = token(&key, "user:admin", "acme", &[PUBLISH_CAP]);

    // A real trusted hello-v2 artifact (tens of KiB) is well under 1 MiB → the size gate passes.
    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/extensions",
                serde_json::to_value(artifact(&id, &sk)).unwrap(),
            ),
            &tok,
        ))
        .await
        .unwrap();

    assert_ne!(
        resp.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "a body under the configured limit must not be rejected for size"
    );
}
