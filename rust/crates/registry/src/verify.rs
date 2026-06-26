//! `verify_artifact` — the registry's trust gate and the slice's only new crypto surface (registry
//! scope; flagged loudly per the non-negotiables). It answers exactly one question: *are these the
//! bytes an allow-listed publisher signed?*
//!
//! Two checks, both must pass:
//!   1. **Integrity** — recompute the SHA-256 content digest over `(manifest_toml, wasm)` and confirm
//!      it equals the artifact's claimed `digest_hex`. Catches a tampered manifest or wasm.
//!   2. **Authenticity** — verify the Ed25519 `signature` over that 32-byte digest against the
//!      `VerifyingKey` the workspace allow-lists for `publisher_key_id`. Catches an unsigned or
//!      foreign-key artifact.
//!
//! Reuses the **`ed25519-dalek` idiom verbatim** from `lb_auth::keypair`/`verify` — no JWT/COSE
//! library, no second crypto stack, so there is no cross-library key-encoding seam (the same reason
//! auth signs tokens directly; debugging/auth/valid-token-fails-verification.md). On any failure it
//! returns [`RegistryError::Unverified`] and mints **no** [`VerifiedArtifact`] — so the cache (which
//! takes only a `VerifiedArtifact`) can never receive unverified bytes. That is the verify-before-cache
//! guarantee, enforced by the type system rather than call ordering.

use std::collections::HashMap;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use crate::digest::{digest, digest_hex};
use crate::error::RegistryError;
use crate::model::{Artifact, VerifiedArtifact};

/// A publisher's Ed25519 verifying key, by 32 public-key bytes. The workspace's "who may I install
/// from" allow-list maps `publisher_key_id -> PublisherKey`. (S7-first: a caller-supplied fixture,
/// the same shape S4's `admin_approved` took; durable storage + rotation are deferred — registry
/// scope open questions.)
pub type TrustedKeys = HashMap<String, PublisherKey>;

/// A publisher verifying key. Wraps the 32 raw Ed25519 public-key bytes; construction validates them
/// so a malformed key is rejected at the allow-list boundary, not mid-verification.
#[derive(Debug, Clone)]
pub struct PublisherKey {
    inner: VerifyingKey,
}

impl PublisherKey {
    /// Build from 32 raw Ed25519 public-key bytes. `Err(Malformed)` if they are not a valid point.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, RegistryError> {
        VerifyingKey::from_bytes(bytes)
            .map(|inner| Self { inner })
            .map_err(|e| RegistryError::Malformed(format!("publisher key: {e}")))
    }
}

/// Verify `artifact` against the workspace's `trusted` publisher keys. On success, returns a
/// [`VerifiedArtifact`] — the only constructor of that newtype, so a verified value *proves* both
/// checks passed. On any failure, [`RegistryError::Unverified`] and nothing is minted.
pub fn verify_artifact(
    artifact: Artifact,
    trusted: &TrustedKeys,
) -> Result<VerifiedArtifact, RegistryError> {
    // 1. Integrity: recompute the digest and confirm the claim. A mismatch is a tamper.
    let computed = digest(&artifact.manifest_toml, &artifact.wasm);
    if digest_hex(&computed) != artifact.digest_hex {
        return Err(RegistryError::Unverified);
    }

    // 2. Authenticity: the signature must verify under an allow-listed key. An unknown key id, a
    //    malformed signature, or a signature from another key all collapse to Unverified — no signal
    //    about which (a foreign artifact learns nothing about the allow-list).
    let key = trusted
        .get(&artifact.publisher_key_id)
        .ok_or(RegistryError::Unverified)?;
    let sig = Signature::from_slice(&artifact.signature).map_err(|_| RegistryError::Unverified)?;
    key.inner
        .verify(&computed, &sig)
        .map_err(|_| RegistryError::Unverified)?;

    Ok(VerifiedArtifact::new(artifact))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    // Deterministic publisher key from a fixed seed (testing §3 — no random key in test logic).
    fn publisher(seed: u8) -> (String, SigningKey, PublisherKey) {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
        (format!("pub-{seed}"), sk, pk)
    }

    fn sign(manifest: &str, wasm: &[u8], key_id: &str, sk: &SigningKey) -> Artifact {
        let d = digest(manifest, wasm);
        Artifact {
            ext_id: "hello".into(),
            version: "0.1.0".into(),
            manifest_toml: manifest.into(),
            wasm: wasm.to_vec(),
            digest_hex: digest_hex(&d),
            publisher_key_id: key_id.into(),
            signature: sk.sign(&d).to_bytes().to_vec(),
        }
    }

    #[test]
    fn verifies_a_correctly_signed_artifact() {
        let (id, sk, pk) = publisher(1);
        let trusted = TrustedKeys::from([(id.clone(), pk)]);
        let art = sign("id=\"hello\"", b"\0asm", &id, &sk);
        let verified = verify_artifact(art.clone(), &trusted).expect("verifies");
        assert_eq!(verified.artifact().ext_id, "hello");
    }

    #[test]
    fn rejects_tampered_wasm() {
        let (id, sk, pk) = publisher(2);
        let trusted = TrustedKeys::from([(id.clone(), pk)]);
        let mut art = sign("id=\"hello\"", b"\0asm", &id, &sk);
        art.wasm = b"\0asn".to_vec(); // bytes changed; digest no longer matches the signed one
        assert_eq!(
            verify_artifact(art, &trusted).unwrap_err(),
            RegistryError::Unverified
        );
    }

    #[test]
    fn rejects_tampered_manifest() {
        let (id, sk, pk) = publisher(3);
        let trusted = TrustedKeys::from([(id.clone(), pk)]);
        let mut art = sign("id=\"hello\"", b"\0asm", &id, &sk);
        // Inflate the requested caps after signing — the digest binds the manifest, so it's caught.
        art.manifest_toml = "id=\"hello\"\nrequest=[\"secret:*\"]".into();
        assert_eq!(
            verify_artifact(art, &trusted).unwrap_err(),
            RegistryError::Unverified
        );
    }

    #[test]
    fn rejects_unsigned_artifact() {
        let (id, sk, pk) = publisher(4);
        let trusted = TrustedKeys::from([(id.clone(), pk)]);
        let mut art = sign("id=\"hello\"", b"\0asm", &id, &sk);
        art.signature = vec![0u8; 64]; // a zero signature is not a valid signature over the digest
        assert_eq!(
            verify_artifact(art, &trusted).unwrap_err(),
            RegistryError::Unverified
        );
    }

    #[test]
    fn rejects_signature_from_untrusted_key() {
        let (id, sk, _pk) = publisher(5);
        let (_other_id, _other_sk, other_pk) = publisher(6);
        // The artifact is correctly signed by key 5, but the workspace only trusts key 6.
        let trusted = TrustedKeys::from([("pub-6".to_string(), other_pk)]);
        let art = sign("id=\"hello\"", b"\0asm", &id, &sk);
        assert_eq!(
            verify_artifact(art, &trusted).unwrap_err(),
            RegistryError::Unverified
        );
    }

    #[test]
    fn rejects_unknown_key_id() {
        let (id, sk, _pk) = publisher(7);
        let trusted = TrustedKeys::new(); // nobody trusted
        let art = sign("id=\"hello\"", b"\0asm", &id, &sk);
        assert_eq!(
            verify_artifact(art, &trusted).unwrap_err(),
            RegistryError::Unverified
        );
    }

    #[test]
    fn malformed_publisher_key_is_rejected_at_the_boundary() {
        // An all-`0x02` fill does not decompress to a valid Ed25519 curve point, so `from_bytes`
        // rejects it (it validates the encoding). The allow-list refuses a malformed key here, before
        // any artifact is verified against it — a packaging bug fails loud at the trust boundary.
        assert!(matches!(
            PublisherKey::from_bytes(&[0x02; 32]),
            Err(RegistryError::Malformed(_))
        ));
    }
}
