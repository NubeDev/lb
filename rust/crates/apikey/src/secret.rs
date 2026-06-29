//! High-entropy key id + secret generation (api-keys scope). The id is short (8 random bytes → ~13
//! base32 chars) and only needs workspace uniqueness; the secret is 32 random bytes (~52 base32
//! chars) of full entropy so a fast keyed hash is correct (not a slow KDF). Both are Crockford
//! base32, so neither contains a `.` or `_` — the bearer grammar stays delimiter-safe.

use rand::RngCore;

use crate::crockford::encode;

/// The number of random bytes in a key id. 8 bytes (64 bits) — collision-resistant per workspace.
const ID_BYTES: usize = 8;

/// The number of random bytes in a key secret. 32 bytes (256 bits) — full entropy.
const SECRET_BYTES: usize = 32;

/// Generate a fresh, workspace-unique key id (Crockford base32, no padding).
pub fn generate_id() -> String {
    encode(&random_bytes(ID_BYTES))
}

/// Generate a fresh high-entropy secret (Crockford base32, no padding). This is the value hashed and
/// shown exactly once at create; it is never recoverable from the stored `key_hash`.
pub fn generate_secret() -> String {
    encode(&random_bytes(SECRET_BYTES))
}

/// `n` cryptographically-random bytes from the thread CSPRNG.
fn random_bytes(n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crockford::is_valid;

    #[test]
    fn id_and_secret_are_valid_crockford_and_distinct_lengths() {
        let id = generate_id();
        let secret = generate_secret();
        assert!(is_valid(&id));
        assert!(is_valid(&secret));
        assert!(
            secret.len() > id.len(),
            "secret should be longer than the id"
        );
    }

    #[test]
    fn generation_is_non_deterministic() {
        // Two draws are (overwhelmingly) not identical — a flake here would indicate a seeded RNG.
        assert_ne!(generate_secret(), generate_secret());
        assert_ne!(generate_id(), generate_id());
    }
}
