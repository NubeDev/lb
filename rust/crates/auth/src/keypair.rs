//! The node signing key — Ed25519 (README §6.6: the hub signs, every node verifies offline).
//!
//! Tokens are signed/verified with `ed25519-dalek` directly (no JWT library), so there is no
//! cross-library key-encoding seam — see debugging/auth/valid-token-fails-verification.md for
//! why that matters. Custody across roles is README §13's open question; S1 generates one
//! key in-process for the solo node.

use ed25519_dalek::{Signature, Signer, SigningKey as DalekKey, Verifier, VerifyingKey};

/// A node's Ed25519 key. Holds the private half; verifies with the public half.
#[derive(Clone)]
pub struct SigningKey {
    inner: DalekKey,
}

impl SigningKey {
    /// Generate a fresh key (solo node, S1). Uses the OS RNG.
    pub fn generate() -> Self {
        let mut rng = rand::rngs::OsRng;
        Self {
            inner: DalekKey::generate(&mut rng),
        }
    }

    /// Reconstruct from the 32-byte seed (e.g. loaded from the keychain/secret store later).
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Self {
            inner: DalekKey::from_bytes(seed),
        }
    }

    /// Sign a message (the JWS signing input). Crate-internal — only `mint` calls it.
    pub(crate) fn sign(&self, message: &[u8]) -> Signature {
        self.inner.sign(message)
    }

    /// Verify a signature over `message`. Crate-internal — only `verify` calls it.
    pub(crate) fn verify(&self, message: &[u8], sig: &Signature) -> bool {
        let vk: VerifyingKey = self.inner.verifying_key();
        vk.verify(message, sig).is_ok()
    }
}
