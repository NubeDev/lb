//! The **zip-transport** half of `POST /extensions` (extension-artifact-upload-size fix):
//! `publish_install_test.rs` proves the JSON path end to end; this file proves the zip path is
//! trust-equivalent (same verify/install outcome, same real `hello-v2` component made callable) AND
//! proves the actual regression that motivated it — a body that 413s on the JSON path at a given
//! ceiling must succeed on the zip path at the SAME ceiling, because the zip payload never carries
//! the JSON decimal-int-array's ~4-8x inflation.
//!
//! No mocks (CLAUDE.md rule #9): real routes, the real `hello-v2` wasm, the real
//! `lb_registry::artifact_to_zip` codec.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::{bearer, gateway, json_post_sized, token, zip_post_sized};
use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_registry::{artifact_to_zip, digest, digest_hex, Artifact, PublisherKey, TrustedKeys};
use lb_role_gateway::{router, Gateway};
use serde_json::{json, Value};
use tower::ServiceExt;

const MANIFEST: &str = include_str!("../../../extensions/hello-v2/extension.toml");
const WASM: &[u8] =
    include_bytes!("../../../extensions/hello-v2/target/wasm32-wasip2/release/hello_v2_ext.wasm");

const PUBLISH_CAP: &str = "mcp:ext.publish:call";
const ECHO_CAP: &str = "mcp:hello.echo:call";

fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

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

/// The zip path is trust-equivalent to the JSON path: same 204, same install, same load, same
/// callable tool — proving `Content-Type: application/zip` and the default JSON path converge on
/// the exact same `ext_publish` outcome, not a parallel/weaker one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn zip_publish_installs_and_loads_a_trusted_artifact_so_it_is_callable() {
    let (gw, key) = gateway().await;
    let (id, sk, trusted) = publisher(7);
    let gw = Gateway::new(Arc::clone(&gw.node), key.clone(), common::NOW).with_trusted(trusted);
    let tok = token(&key, "user:admin", "nube", &[PUBLISH_CAP, ECHO_CAP]);

    let zip_bytes = artifact_to_zip(&artifact(&id, &sk)).expect("pack zip");
    let resp = router(gw.clone())
        .oneshot(bearer(zip_post_sized("/extensions", zip_bytes), &tok))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "trusted zip publish installs"
    );

    let resp = router(gw)
        .oneshot(bearer(
            common::json_post(
                "/mcp/call",
                json!({ "tool": "hello.echo", "args": { "msg": "hi" } }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the zip-published tool is callable"
    );
    let out: Value = common::json_body(resp).await;
    assert_eq!(out["v"], 2, "the v2 component is what got loaded: {out}");
}

/// An untrusted publisher over the zip path is refused exactly like the JSON path — proves
/// `artifact_from_zip` makes no trust decision of its own; `verify_artifact_with` still gates it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn zip_an_untrusted_publisher_is_422_and_nothing_is_installed() {
    let (gw, key) = gateway().await;
    let (_trusted_id, _trusted_sk, trusted) = publisher(7);
    let (foreign_id, foreign_sk, _) = publisher(9);
    let gw = Gateway::new(Arc::clone(&gw.node), key.clone(), common::NOW).with_trusted(trusted);
    let tok = token(&key, "user:admin", "nube", &[PUBLISH_CAP, ECHO_CAP]);

    let zip_bytes = artifact_to_zip(&artifact(&foreign_id, &foreign_sk)).expect("pack zip");
    let resp = router(gw)
        .oneshot(bearer(zip_post_sized("/extensions", zip_bytes), &tok))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "untrusted zip publish is 422"
    );
}

/// A caller without `ext.publish` is refused server-side over the zip path too — the capability gate
/// is `ext_publish`'s, reached identically by both transports.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn zip_publish_without_the_capability_is_denied_server_side() {
    let (gw, key) = gateway().await;
    let (id, sk, trusted) = publisher(7);
    let gw = Gateway::new(Arc::clone(&gw.node), key.clone(), common::NOW).with_trusted(trusted);
    let tok = token(&key, "user:mallory", "nube", &["bus:chan/*:pub"]);

    let zip_bytes = artifact_to_zip(&artifact(&id, &sk)).expect("pack zip");
    let resp = router(gw)
        .oneshot(bearer(zip_post_sized("/extensions", zip_bytes), &tok))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "no cap → 403 server-side, zip path too"
    );
}

/// A malformed zip body (not even a readable archive) is a `422`, not a panic or a 500 — the
/// `RegistryError::Transport` → gateway status mapping, exercised through the real route.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn zip_a_malformed_body_is_422_not_a_crash() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:admin", "nube", &[PUBLISH_CAP]);

    let resp = router(gw)
        .oneshot(bearer(
            zip_post_sized("/extensions", b"not a zip at all".to_vec()),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "a malformed zip artifact is 422, never a crash"
    );
}

/// **The regression test that would have caught the real incident.** Pin the SAME low ceiling on
/// both requests. A ~2 MiB `wasm` field JSON-encodes to a body that clears a 1 MiB ceiling by a wide
/// margin (decimal-int-array inflation) and must 413 — reproducing, at test scale, the exact failure
/// a real 191 MB native binary hit in production. The identical payload over the zip transport stays
/// close to 1x size and must clear the SAME ceiling and reach verify (not 413) — proving the fix at
/// the precise seam the incident hit, not just at the codec-unit level.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn json_413s_at_a_ceiling_the_same_payload_clears_over_zip() {
    let ceiling: u64 = 1024 * 1024; // 1 MiB — small on purpose, for a fast test.
                                    // 700 KiB raw payload: comfortably UNDER `ceiling` at zip's ~1x size (so the zip path must
                                    // clear it), but JSON's decimal-int-array encoding of an all-zero payload is a reliable ~2x
                                    // (`"0,"` per byte) — 700 KiB * 2 ≈ 1.37 MiB, comfortably OVER `ceiling` (so the JSON path must
                                    // 413). The two encodings of the SAME bytes land on opposite sides of the same ceiling.
    let wasm_bytes = 700 * 1024;

    let (id, sk, trusted) = publisher(7);
    let (gw, key) = gateway().await;
    let json_gw = Gateway::new(Arc::clone(&gw.node), key.clone(), common::NOW)
        .with_trusted(trusted.clone())
        .with_max_extension_upload_bytes(ceiling);
    let tok = token(&key, "user:admin", "nube", &[PUBLISH_CAP]);

    let json_resp = router(json_gw)
        .oneshot(bearer(
            json_post_sized(
                "/extensions",
                serde_json::to_value(padded_artifact(wasm_bytes, &id, &sk)).unwrap(),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        json_resp.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "JSON encoding of a {wasm_bytes}-byte payload must 413 at a {ceiling}-byte ceiling \
         (reproducing the real incident at test scale)"
    );

    let (gw2, key2) = gateway().await;
    let zip_gw = Gateway::new(Arc::clone(&gw2.node), key2.clone(), common::NOW)
        .with_trusted(trusted)
        .with_max_extension_upload_bytes(ceiling);
    let tok2 = token(&key2, "user:admin", "nube", &[PUBLISH_CAP]);
    let zip_bytes = artifact_to_zip(&padded_artifact(wasm_bytes, &id, &sk)).expect("pack zip");
    assert!(
        (zip_bytes.len() as u64) < ceiling,
        "the whole point: the SAME {wasm_bytes}-byte payload as a zip artifact ({} bytes) must fit \
         under the {ceiling}-byte ceiling that rejected its JSON encoding",
        zip_bytes.len()
    );

    let zip_resp = router(zip_gw)
        .oneshot(bearer(zip_post_sized("/extensions", zip_bytes), &tok2))
        .await
        .unwrap();
    assert_ne!(
        zip_resp.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "the identical payload over zip must clear the same ceiling that rejected it as JSON"
    );
}
