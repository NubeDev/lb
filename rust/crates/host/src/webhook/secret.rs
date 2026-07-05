//! High-entropy shared-secret generation for `signature`-mode webhooks (webhooks scope). Mirrors
//! [`lb_apikey::generate_secret`] — 32 random bytes (~52 Crockford base32 chars) of full entropy,
//! so a fast keyed hash (HMAC-SHA256) is correct over the bytes (not a slow KDF). Stored in
//! `lb-secrets` at `webhook/{id}` under Workspace visibility; shown once at create, never
//! recoverable from the stored value.

use rand::RngCore;

use lb_apikey::generate_secret as apikey_secret;

/// Generate a fresh high-entropy shared secret for a `signature`-mode webhook. Reuses the apikey
/// secret generator verbatim — same entropy, same Crockford base32 shape (no `.`/`_`), so it is
/// safe to print, paste into a provider's webhook config, and compare byte-for-byte if needed.
///
/// The generator is the SAME one `apikey_create` uses; the difference is *where the value lives*
/// (`lb-secrets` vs the apikey hash row) and *how it is verified* (HMAC over body vs hash compare).
pub fn generate_shared_secret() -> String {
    // Reuse the apikey generator: same entropy + shape, one code path. We do NOT add a prefix
    // (`lbk_`/`lbh_`) — the shared secret is opaque to the caller, who pastes it into their
    // provider's "secret" field; a prefix would just be noise.
    apikey_secret()
}

/// `n` cryptographically-random bytes from the thread CSPRNG — exposed for any future per-hook
/// salt/nonce need. Currently unused by the v1 path (kept for symmetry with `lb_apikey::secret`).
#[allow(dead_code)]
pub(crate) fn random_bytes(n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_secret_is_non_empty_and_non_deterministic() {
        let a = generate_shared_secret();
        let b = generate_shared_secret();
        assert!(!a.is_empty());
        assert_ne!(a, b, "two draws must not collide");
        assert!(a.len() >= 50, "expected ~52 base32 chars, got {}", a.len());
    }
}
