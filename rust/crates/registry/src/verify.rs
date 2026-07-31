//! `verify_artifact` — the registry's trust gate and the slice's only new crypto surface (registry
//! scope; flagged loudly per the non-negotiables). It answers exactly one question: *are these the
//! bytes an allow-listed publisher signed?*
//!
//! Two **independent** checks:
//!   1. **Integrity** — recompute the SHA-256 content digest over `(manifest_toml, wasm)` and confirm
//!      it equals the artifact's claimed `digest_hex`. Catches a tampered manifest or wasm. This check
//!      is **unconditional**: no caller, no configuration, and no environment can skip it. A corrupt
//!      or truncated artifact is [`RegistryError::Unverified`], always.
//!   2. **Authenticity** — verify the Ed25519 `signature` over that 32-byte digest against the
//!      `VerifyingKey` the workspace allow-lists for `publisher_key_id`. Catches an unsigned or
//!      foreign-key artifact. This check is **waivable** by the caller passing
//!      [`Authenticity::WaivedUntrustedKey`] (see below).
//!
//! They are deliberately not one boolean. Integrity answers *are these the bytes that were packed?*;
//! authenticity answers *who packed them?*. The escape hatch waives only the second — a bench node
//! with the hatch on still refuses a corrupted upload.
//!
//! ## The authenticity waiver
//!
//! [`Authenticity`] is a **parameter, never read from the environment here**. This crate holds no
//! policy: it cannot see an env var, cannot default to permissive, and cannot be reconfigured at a
//! distance. A caller that wants the waiver must name it at the call site, which is what keeps the
//! hatch auditable by `grep WaivedUntrustedKey`. The role layer (`lb-gateway`'s `session::trusted`,
//! `lb-node`'s `Config`) is where an operator's `LB_EXT_UNTRUSTED_KEY=allow` is parsed and mapped.
//!
//! Note [`verify_artifact`] itself keeps its two-argument shape and hard-codes
//! [`Authenticity::Required`]. That is the fail-closed default made structural: an existing or future
//! call site gets the full gate unless it is rewritten to call [`verify_artifact_with`]. Adding a
//! caller can never *accidentally* weaken the gate.
//!
//! Reuses the **`ed25519-dalek` idiom verbatim** from `lb_auth::keypair`/`verify` — no JWT/COSE
//! library, no second crypto stack, so there is no cross-library key-encoding seam (the same reason
//! auth signs tokens directly; debugging/auth/valid-token-fails-verification.md). On any failure it
//! returns [`RegistryError::Unverified`] and mints **no** [`VerifiedArtifact`] — so the cache (which
//! takes only a `VerifiedArtifact`) can never receive artifacts that failed integrity. That is the
//! verify-before-cache guarantee, enforced by the type system rather than call ordering, and the
//! waiver does not weaken it: a waived artifact still passes through this one function, is still the
//! sole mint of the newtype, and **carries the waiver with it** ([`VerifiedArtifact::authenticity`])
//! so the fact is inspectable downstream rather than erased.

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

/// Whether the **authenticity** half of the gate is enforced. Integrity is not represented here
/// because it is not optional — see the module docs for why the two are not one boolean.
///
/// This is a caller-supplied policy value. The registry crate never derives it from the environment;
/// a role crate maps an operator knob onto it. [`Default`] is [`Authenticity::Required`], so a
/// derived/`..Default::default()` construction fails closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Authenticity {
    /// The signature must verify against an allow-listed publisher key. Today's behaviour, and the
    /// only posture any production node should run.
    #[default]
    Required,
    /// **Escape hatch — development only.** Accept an artifact whose `publisher_key_id` is not in the
    /// allow-list, or whose signature does not verify. Integrity is still enforced. A node in this
    /// posture will install code from any publisher, so it must never face an untrusted network; the
    /// gateway logs a warning at boot and on every waived artifact, and reports it on `GET /health`.
    WaivedUntrustedKey,
}

impl Authenticity {
    /// `true` when the authenticity check is being skipped — the condition the loud surfaces report.
    pub fn is_waived(self) -> bool {
        matches!(self, Self::WaivedUntrustedKey)
    }
}

/// Verify `artifact` against the workspace's `trusted` publisher keys, with the authenticity check
/// **enforced** ([`Authenticity::Required`]).
///
/// This is the fail-closed entry point and the one every call site should use. The escape hatch is
/// deliberately not reachable from here: a caller that wants it must say so by name via
/// [`verify_artifact_with`], so no future edit can weaken the gate by omission.
pub fn verify_artifact(
    artifact: Artifact,
    trusted: &TrustedKeys,
) -> Result<VerifiedArtifact, RegistryError> {
    verify_artifact_with(artifact, trusted, Authenticity::Required)
}

/// Verify `artifact`, choosing whether the authenticity half of the gate is enforced.
///
/// **Integrity always runs**, before and independent of `authenticity`: the digest is recomputed over
/// `(manifest_toml, wasm)` and compared to the claim, and a mismatch is [`RegistryError::Unverified`]
/// regardless of the waiver. Only the signature/allow-list check is skippable.
///
/// On success returns a [`VerifiedArtifact`] — still the *only* constructor of that newtype, so the
/// verify-before-cache seam is intact. The returned value records which posture minted it, so a
/// waived artifact is distinguishable downstream rather than silently indistinguishable from a fully
/// verified one. On any failure, [`RegistryError::Unverified`] and nothing is minted.
pub fn verify_artifact_with(
    artifact: Artifact,
    trusted: &TrustedKeys,
    authenticity: Authenticity,
) -> Result<VerifiedArtifact, RegistryError> {
    // 1. Integrity: recompute the digest and confirm the claim. A mismatch is a tamper.
    //    Unconditional and first — `authenticity` is not consulted until after this returns, so
    //    there is no arrangement of arguments under which a corrupt artifact is accepted.
    let computed = digest(&artifact.manifest_toml, &artifact.wasm);
    if digest_hex(&computed) != artifact.digest_hex {
        return Err(RegistryError::Unverified);
    }

    // 2. Authenticity: the signature must verify under an allow-listed key. An unknown key id, a
    //    malformed signature, or a signature from another key all collapse to Unverified — no signal
    //    about which (a foreign artifact learns nothing about the allow-list).
    //
    //    ...unless the operator waived it. The early return skips ONLY this block; step 1 already
    //    ran and already passed, which is precisely the property the escape hatch promises.
    if authenticity.is_waived() {
        return Ok(VerifiedArtifact::new(artifact, authenticity));
    }
    let key = trusted
        .get(&artifact.publisher_key_id)
        .ok_or(RegistryError::Unverified)?;
    let sig = Signature::from_slice(&artifact.signature).map_err(|_| RegistryError::Unverified)?;
    key.inner
        .verify(&computed, &sig)
        .map_err(|_| RegistryError::Unverified)?;

    Ok(VerifiedArtifact::new(artifact, Authenticity::Required))
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

    // ---- The authenticity escape hatch (`Authenticity::WaivedUntrustedKey`). ----------------
    //
    // The property under test throughout: the waiver skips *who signed it*, never *are these the
    // bytes that were packed*. Integrity must survive the hatch being on.

    #[test]
    fn waived_accepts_an_artifact_signed_by_a_key_that_is_not_allow_listed() {
        let (id, sk, _pk) = publisher(8);
        let trusted = TrustedKeys::new(); // nobody trusted — the bench-node case
        let art = sign("id=\"hello\"", b"\0asm", &id, &sk);
        let verified = verify_artifact_with(art, &trusted, Authenticity::WaivedUntrustedKey)
            .expect("the waiver accepts a foreign publisher key");
        assert_eq!(verified.artifact().ext_id, "hello");
        assert_eq!(
            verified.authenticity(),
            Authenticity::WaivedUntrustedKey,
            "the weakened posture travels with the value; it is not erased at the boundary"
        );
    }

    #[test]
    fn waived_still_rejects_a_tampered_wasm() {
        // THE test that matters: integrity is not waivable. A corrupt/truncated upload is refused
        // even with the hatch fully open, because the digest check runs before `authenticity` is
        // ever consulted.
        let (id, sk, _pk) = publisher(9);
        let trusted = TrustedKeys::new();
        let mut art = sign("id=\"hello\"", b"\0asm", &id, &sk);
        art.wasm = b"\0asn".to_vec();
        assert_eq!(
            verify_artifact_with(art, &trusted, Authenticity::WaivedUntrustedKey).unwrap_err(),
            RegistryError::Unverified
        );
    }

    #[test]
    fn waived_still_rejects_a_tampered_manifest() {
        // The cap-inflation attack specifically: the manifest is digest-bound, and the waiver does
        // not touch the digest, so inflating `request=[...]` after signing is still caught.
        let (id, sk, _pk) = publisher(10);
        let trusted = TrustedKeys::new();
        let mut art = sign("id=\"hello\"", b"\0asm", &id, &sk);
        art.manifest_toml = "id=\"hello\"\nrequest=[\"secret:*\"]".into();
        assert_eq!(
            verify_artifact_with(art, &trusted, Authenticity::WaivedUntrustedKey).unwrap_err(),
            RegistryError::Unverified
        );
    }

    #[test]
    fn waived_still_rejects_a_claimed_digest_that_is_simply_wrong() {
        // Not a tamper of the payload but of the claim itself — a truncated/garbled upload whose
        // `digest_hex` no longer describes its bytes. Same answer.
        let (id, sk, _pk) = publisher(11);
        let trusted = TrustedKeys::new();
        let mut art = sign("id=\"hello\"", b"\0asm", &id, &sk);
        art.digest_hex = "00".repeat(32);
        assert_eq!(
            verify_artifact_with(art, &trusted, Authenticity::WaivedUntrustedKey).unwrap_err(),
            RegistryError::Unverified
        );
    }

    #[test]
    fn required_is_the_default_and_matches_the_two_argument_entry_point() {
        // `verify_artifact` must be exactly `verify_artifact_with(.., Required)`: the fail-closed
        // default is structural, so no future call site weakens the gate by omission.
        let (id, sk, _pk) = publisher(12);
        let trusted = TrustedKeys::new();
        let art = sign("id=\"hello\"", b"\0asm", &id, &sk);
        assert_eq!(
            verify_artifact(art.clone(), &trusted).unwrap_err(),
            RegistryError::Unverified,
            "the two-arg entry point still enforces authenticity"
        );
        assert_eq!(
            verify_artifact_with(art, &trusted, Authenticity::Required).unwrap_err(),
            RegistryError::Unverified
        );
        assert_eq!(Authenticity::default(), Authenticity::Required);
    }

    #[test]
    fn a_fully_verified_artifact_records_that_authenticity_was_required() {
        let (id, sk, pk) = publisher(13);
        let trusted = TrustedKeys::from([(id.clone(), pk)]);
        let art = sign("id=\"hello\"", b"\0asm", &id, &sk);
        let verified = verify_artifact(art, &trusted).expect("verifies");
        assert_eq!(verified.authenticity(), Authenticity::Required);
        assert!(!verified.authenticity().is_waived());
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
