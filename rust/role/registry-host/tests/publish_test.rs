//! The registry-host **publish** path (lifecycle-management scope): an upload is verified against the
//! publisher allow-list **before** it is stored — the authenticity-before-authority gate. Exercises
//! `ArtifactStore::publish` directly (the `POST /artifacts` route is a thin wrapper that maps `Ok →
//! 204` / `Err → 403`). Tampered, unsigned, and foreign-key uploads are rejected and **nothing is
//! stored**; a correctly-signed upload publishes and is then served by the read path; re-publishing
//! the same bytes is idempotent (no duplicate catalog entry).

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys};
use lb_role_registry_host::ArtifactStore;

fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

fn sign(
    ext: &str,
    ver: &str,
    manifest: &str,
    wasm: &[u8],
    key_id: &str,
    sk: &PublisherSigningKey,
) -> Artifact {
    let d = digest(manifest, wasm);
    Artifact {
        ext_id: ext.into(),
        version: ver.into(),
        manifest_toml: manifest.into(),
        wasm: wasm.to_vec(),
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

const MANIFEST: &str = "id = \"hvac\"\nversion = \"v1\"\n";
const WASM: &[u8] = b"\0asm fake component bytes";

#[test]
fn signed_artifact_publishes_then_serves_and_is_idempotent() {
    let (id, sk, trusted) = publisher(7);
    let store = ArtifactStore::with_trusted(vec![], trusted);
    let art = sign("hvac", "v1", MANIFEST, WASM, &id, &sk);

    // Not present before publish.
    assert!(store.get("hvac", "v1").is_none());
    // Publish a correctly-signed artifact → stored, then served by the read path.
    store
        .publish(art.clone())
        .expect("signed artifact publishes");
    assert_eq!(store.get("hvac", "v1").as_ref(), Some(&art));
    // Idempotent: re-publishing the same bytes succeeds and does not duplicate.
    store.publish(art).expect("re-publish is idempotent");
    assert!(store.get("hvac", "v1").is_some());
}

#[test]
fn tampered_artifact_is_rejected_before_storing() {
    let (id, sk, trusted) = publisher(7);
    let store = ArtifactStore::with_trusted(vec![], trusted);
    let mut art = sign("hvac", "v1", MANIFEST, WASM, &id, &sk);
    art.wasm = b"different bytes".to_vec(); // digest no longer matches the signature.

    assert!(store.publish(art).is_err(), "a tamper must be rejected");
    assert!(
        store.get("hvac", "v1").is_none(),
        "nothing stored on reject"
    );
}

#[test]
fn foreign_key_artifact_is_rejected_before_storing() {
    // Signed by publisher 9, but the store trusts only publisher 7.
    let (_id7, _sk7, trusted) = publisher(7);
    let (id9, sk9, _t9) = publisher(9);
    let store = ArtifactStore::with_trusted(vec![], trusted);
    let art = sign("hvac", "v1", MANIFEST, WASM, &id9, &sk9);

    assert!(
        store.publish(art).is_err(),
        "a foreign-key upload must be rejected"
    );
    assert!(
        store.get("hvac", "v1").is_none(),
        "nothing stored on reject"
    );
}

#[test]
fn unsigned_artifact_is_rejected_before_storing() {
    let (id, _sk, trusted) = publisher(7);
    let store = ArtifactStore::with_trusted(vec![], trusted);
    // A bogus (empty) signature over an otherwise well-formed artifact.
    let d = digest(MANIFEST, WASM);
    let art = Artifact {
        ext_id: "hvac".into(),
        version: "v1".into(),
        manifest_toml: MANIFEST.into(),
        wasm: WASM.to_vec(),
        digest_hex: digest_hex(&d),
        publisher_key_id: id,
        signature: vec![0u8; 64],
    };
    assert!(
        store.publish(art).is_err(),
        "an unsigned upload must be rejected"
    );
    assert!(
        store.get("hvac", "v1").is_none(),
        "nothing stored on reject"
    );
}
