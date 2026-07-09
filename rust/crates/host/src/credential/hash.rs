//! Argon2id password hashing + constant-time verification (login-hardening scope). One responsibility:
//! turn a plaintext secret into a PHC hash string, and check a plaintext against a stored PHC string.
//! No store, no caps — the pure crypto seam the credential verbs call.
//!
//! Params: argon2's library defaults (argon2id, v19). The scope flags "argon2 cost vs login latency
//! on edge devices — pick deliberately"; the defaults are a sane middle and can be tuned in one place
//! here without touching any caller. The salt is random per hash (embedded in the PHC string).

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand::rngs::OsRng;

/// Hash `plaintext` into an argon2id PHC string (random salt embedded). `Err` only on an internal
/// hashing failure (never on input shape — an empty secret hashes fine; emptiness is rejected by the
/// verb, not here).
pub fn hash_secret(plaintext: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plaintext.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

/// Verify `plaintext` against a stored argon2 PHC string in constant time. Returns `Ok(true)` on a
/// match, `Ok(false)` on a mismatch, `Err` only if `phc` is not a parseable argon2 hash (a corrupt
/// record — treated as a hard error by the caller, never as "password ok"). Never leaks timing on the
/// compare (argon2's verifier is constant-time).
pub fn verify_secret(plaintext: &str, phc: &str) -> Result<bool, String> {
    let parsed = PasswordHash::new(phc).map_err(|e| e.to_string())?;
    match Argon2::default().verify_password(plaintext.as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_verifies_against_its_own_hash_and_nothing_else() {
        let phc = hash_secret("hunter2").expect("hash");
        assert!(verify_secret("hunter2", &phc).expect("verify ok"));
        assert!(!verify_secret("wrong", &phc).expect("verify runs"));
    }

    #[test]
    fn each_hash_has_a_fresh_salt() {
        // Same plaintext → different PHC strings (random salt), both verifying.
        let a = hash_secret("same").unwrap();
        let b = hash_secret("same").unwrap();
        assert_ne!(a, b, "salt makes the hash unique per call");
        assert!(verify_secret("same", &a).unwrap());
        assert!(verify_secret("same", &b).unwrap());
    }

    #[test]
    fn a_corrupt_phc_string_is_an_error_not_a_match() {
        assert!(verify_secret("x", "not-a-phc-string").is_err());
    }
}
