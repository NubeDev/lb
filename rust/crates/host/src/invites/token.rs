//! Token generation + hashing for invites (invites scope). The token is 32 random bytes (Crockford
//! base32, `lbi_`-prefixed) — full entropy, so a fast SHA-256 hash is correct (not a slow KDF).
//! The hash is the record id (`invite:{hash}`) for O(1) lookup on accept. The raw token is shown
//! exactly once at create and is never recoverable from the stored hash.

use lb_apikey::generate_secret;
use sha2::{Digest, Sha256};

/// The invite token prefix (distinguishes from `lbk_` apikey bearers).
pub const TOKEN_PREFIX: &str = "lbi_";

/// Generate a fresh invite token: `lbi_<52-char-crockford-base32>`. Shown once at create.
pub fn generate_token() -> String {
    format!("{TOKEN_PREFIX}{}", generate_secret())
}

/// SHA-256 hash of the raw token (hex). Stored as the record id and the `token_hash` field.
/// A fast hash is correct because the token is 32 random bytes (full entropy) — same reasoning as
/// apikeys (which use HMAC-SHA256; invites don't need the pepper because the token is never a
/// user-chosen password).
pub fn hash_token(token: &str) -> String {
    let hash = Sha256::digest(token.as_bytes());
    hex_encode(&hash)
}

/// Encode bytes as lowercase hex (no external `hex` crate dependency).
fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    out
}

/// Strip the `lbi_` prefix and validate the token shape. Returns `None` for a malformed token.
pub fn validate_token(token: &str) -> Option<&str> {
    token.strip_prefix(TOKEN_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_has_prefix_and_is_unique() {
        let a = generate_token();
        let b = generate_token();
        assert!(a.starts_with(TOKEN_PREFIX));
        assert!(b.starts_with(TOKEN_PREFIX));
        assert_ne!(a, b);
    }

    #[test]
    fn hash_is_deterministic_and_hex() {
        let token = generate_token();
        let h1 = hash_token(&token);
        let h2 = hash_token(&token);
        assert_eq!(h1, h2);
        assert!(h1.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn validate_strips_prefix() {
        let token = generate_token();
        let stripped = validate_token(&token);
        assert!(stripped.is_some());
        assert!(!stripped.unwrap().starts_with(TOKEN_PREFIX));
    }

    #[test]
    fn validate_rejects_bad_prefix() {
        assert!(validate_token("lbk_abc").is_none());
        assert!(validate_token("abc").is_none());
    }
}
