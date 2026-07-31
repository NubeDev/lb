//! The signed **artifact** — the unit of distribution (registry scope, README §6.4).
//!
//! An `Artifact` carries everything a node needs to install an extension *and* to prove the bytes
//! are the publisher's: the `manifest_toml` + `wasm`, the claimed content `digest_hex`, the
//! `publisher_key_id`, and the Ed25519 `signature` over the digest. It is **untrusted** until
//! `verify_artifact` checks it — that is why the cache takes a [`VerifiedArtifact`], not this.

use serde::{Deserialize, Serialize};

use crate::verify::Authenticity;

/// A signed, versioned extension artifact as fetched from a `Source`. UNTRUSTED on arrival: the
/// `digest_hex`/`signature` are *claims* the publisher made; `verify_artifact` is what turns a
/// claim into a fact. Fields are bytes/strings so the record is transport- and store-agnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub ext_id: String,
    pub version: String,
    /// The full `extension.toml` the loader parses. Bound by the digest, so a tampered manifest is
    /// caught even though the grant intersection would also neutralize it (defense in depth).
    pub manifest_toml: String,
    /// The component bytes. In our `kv-mem` build these are stored as a record, not a `DEFINE
    /// BUCKET` blob (the same S4 doc-content path; the bucket swap is a later config change).
    pub wasm: Vec<u8>,
    /// The publisher's claimed content digest, lowercase hex. `verify_artifact` recomputes and
    /// compares — a mismatch is a tamper.
    pub digest_hex: String,
    /// Which publisher key signed this — looked up in the workspace's trusted-key allow-list.
    pub publisher_key_id: String,
    /// Ed25519 signature over the 32-byte digest, 64 bytes. Verified against the publisher key.
    pub signature: Vec<u8>,
}

/// A [`Artifact`] that has passed `verify_artifact`. **This newtype is the load-bearing seam**:
/// `cache_artifact` accepts only a `VerifiedArtifact`, and the only way to construct one is
/// `verify_artifact`/`verify_artifact_with`. So "an artifact reaches the cache only by passing
/// through the verifier" is a *compile-time* guarantee, not a convention the next edit might forget
/// (registry scope, the verify-before-cache risk; the §11.5 "make the class impossible" preference).
///
/// ## Exactly what this type proves
///
/// - **Always**: the content digest was recomputed over `(manifest_toml, wasm)` and matched the
///   artifact's claim. Integrity is unconditional in the verifier, so *every* value of this type has
///   it — a corrupt or truncated artifact can never wear this type.
/// - **Conditionally**: the Ed25519 signature verified against an allow-listed publisher key. True
///   when [`VerifiedArtifact::authenticity`] is [`Authenticity::Required`]; **not** established when
///   it is [`Authenticity::WaivedUntrustedKey`] (the development escape hatch — see `verify`).
///
/// The waiver is carried in the value rather than erased at the boundary on purpose. Collapsing both
/// postures into one indistinguishable type would make this doc comment a lie and leave downstream
/// code (the cache, the loader, the boot bring-up) unable to tell whether the publisher was ever
/// checked. Reading [`VerifiedArtifact::authenticity`] is how a caller reports or refuses it.
#[derive(Debug, Clone)]
pub struct VerifiedArtifact {
    artifact: Artifact,
    authenticity: Authenticity,
}

impl VerifiedArtifact {
    /// Mint a verified artifact. `pub(crate)` on purpose: only the `verify` module (same crate) may
    /// call it, after the integrity check passes and the authenticity check passes *or is waived*.
    /// No other path can fabricate one. `authenticity` records which of those two happened.
    pub(crate) fn new(artifact: Artifact, authenticity: Authenticity) -> Self {
        Self {
            artifact,
            authenticity,
        }
    }

    /// The verified inner artifact — read-only access for the cache/loader.
    pub fn artifact(&self) -> &Artifact {
        &self.artifact
    }

    /// Whether the publisher's signature was actually checked, or waived by the operator's escape
    /// hatch. Integrity passed either way. Callers that surface or log the weakened posture read this.
    pub fn authenticity(&self) -> Authenticity {
        self.authenticity
    }

    /// Consume into the inner artifact (e.g. to persist it).
    pub fn into_artifact(self) -> Artifact {
        self.artifact
    }
}
