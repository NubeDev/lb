//! The signed **artifact** — the unit of distribution (registry scope, README §6.4).
//!
//! An `Artifact` carries everything a node needs to install an extension *and* to prove the bytes
//! are the publisher's: the `manifest_toml` + `wasm`, the claimed content `digest_hex`, the
//! `publisher_key_id`, and the Ed25519 `signature` over the digest. It is **untrusted** until
//! `verify_artifact` checks it — that is why the cache takes a [`VerifiedArtifact`], not this.

use serde::{Deserialize, Serialize};

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

/// A [`Artifact`] that has passed `verify_artifact` — the digest matched and the signature verified
/// against an allow-listed publisher key. **This newtype is the load-bearing seam**: `cache_artifact`
/// accepts only a `VerifiedArtifact`, and the only way to construct one is `verify_artifact`. So
/// "an unverified artifact is never cached" is a *compile-time* guarantee, not a convention the next
/// edit might forget (registry scope, the verify-before-cache risk; the §11.5 "make the class
/// impossible" preference).
#[derive(Debug, Clone)]
pub struct VerifiedArtifact(Artifact);

impl VerifiedArtifact {
    /// Mint a verified artifact. `pub(crate)` on purpose: only `verify_artifact` (same crate) may
    /// call it, after the digest + signature checks pass. No other path can fabricate one.
    pub(crate) fn new(artifact: Artifact) -> Self {
        Self(artifact)
    }

    /// The verified inner artifact — read-only access for the cache/loader.
    pub fn artifact(&self) -> &Artifact {
        &self.0
    }

    /// Consume into the inner artifact (e.g. to persist it).
    pub fn into_artifact(self) -> Artifact {
        self.0
    }
}
