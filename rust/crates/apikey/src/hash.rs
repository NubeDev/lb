//! The peppered hash + constant-time compare (api-keys scope). `key_hash = HMAC-SHA256(pepper,
//! secret_field)` — a keyed hash whose input is the **`secret` field alone**, never the full bearer
//! string (a unit test pins that a full-string hash does NOT match). High entropy (32 bytes) means a
//! fast keyed hash is correct here, not a slow KDF; the pepper comes from `lb-secrets`/env, never the
//! DB. The stored form is a 64-char lowercase hex string; comparison is constant-time (the vetted
//! XOR-accumulate the github-webhook verifier uses), never `==`.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Compute the stored hash of a key's `secret` field under `pepper`, as 64-char lowercase hex. The
/// input is the secret field ALONE — never the full bearer string.
pub fn key_hash(pepper: &[u8], secret: &str) -> String {
    let tag = hmac_tag(pepper, secret.as_bytes());
    hex(&tag)
}

/// Constant-time verify that `HMAC(pepper, secret_field) == stored_hex`. Over the secret field only.
/// A malformed `stored_hex` (wrong length / non-hex) compares unequal — never panics.
pub fn verify_hash(pepper: &[u8], secret: &str, stored_hex: &str) -> bool {
    let Some(stored) = decode_hex32(stored_hex) else {
        return false;
    };
    let expected = hmac_tag(pepper, secret.as_bytes());
    constant_time_eq(&stored, &expected)
}

/// Constant-time compare of two hex hash strings (as produced by [`key_hash`]). A malformed or
/// length-mismatched argument compares unequal — never panics. Used by the host's verification cache,
/// which stores a verified hash and must reject a different secret WITHOUT recomputing the HMAC
/// (the cache never holds the secret or the pepper, only the resulting hash).
pub fn hash_matches(stored: &str, presented: &str) -> bool {
    match (decode_hex32(stored), decode_hex32(presented)) {
        (Some(a), Some(b)) => constant_time_eq(&a, &b),
        _ => false,
    }
}

/// The raw HMAC-SHA256 tag of `msg` under `pepper`. HMAC accepts any key length, so this never
/// fails; a `pepper` of any size (including the dev default) yields a stable, comparable tag.
fn hmac_tag(pepper: &[u8], msg: &[u8]) -> [u8; 32] {
    // HMAC accepts any key length, so `new_from_slice` is infallible; a `pepper` of any size yields a
    // stable, comparable tag.
    let mut mac = HmacSha256::new_from_slice(pepper).expect("hmac accepts any key length");
    mac.update(msg);
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    out
}

/// Constant-time byte equality: always touches every byte of `a`, never short-circuits, so the time
/// taken does not reveal how many leading bytes matched. Returns `false` immediately only on a length
/// mismatch (lengths are not secret — both are a fixed 32 here). Mirrors the github-webhook verifier.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    out
}

fn decode_hex32(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let bytes = hex.as_bytes();
    let mut out = [0u8; 32];
    for (i, pair) in bytes.chunks_exact(2).enumerate() {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        out[i] = ((hi << 4) | lo) as u8;
    }
    Some(out)
}

// NOTE: `new_from_slice` is the `hmac::Mac` trait's keyed constructor; `use hmac::Mac` (via
// `hmac::{Hmac, Mac}`) brings it into scope above. HMAC keys are arbitrary-length, so the call is
// infallible.

#[cfg(test)]
mod tests {
    use super::*;

    const PEPPER: &[u8] = b"test-pepper";

    #[test]
    fn hash_round_trips_under_a_fixed_pepper() {
        let h = key_hash(PEPPER, "my-secret");
        assert!(verify_hash(PEPPER, "my-secret", &h));
    }

    #[test]
    fn the_wrong_secret_does_not_verify() {
        let h = key_hash(PEPPER, "my-secret");
        assert!(!verify_hash(PEPPER, "not-my-secret", &h));
    }

    #[test]
    fn the_wrong_pepper_does_not_verify() {
        let h = key_hash(PEPPER, "my-secret");
        assert!(!verify_hash(b"other-pepper", "my-secret", &h));
    }

    #[test]
    fn the_hash_input_is_the_secret_field_only_not_the_full_bearer() {
        // Hashing the full bearer string MUST NOT match a hash of the secret field alone — pinned so
        // no future edit accidentally hashes the whole credential (which would bind the id/ws into
        // the secret and break the O(1) ws-scoped lookup contract).
        let secret = "s3cr3tfield";
        let h = key_hash(PEPPER, secret);
        let full = "lbk_acme.k7f3a.s3cr3tfield";
        assert_ne!(key_hash(PEPPER, full), h);
    }

    #[test]
    fn a_malformed_stored_hash_compares_unequal_without_panicking() {
        let h = key_hash(PEPPER, "s");
        // Malformed stored values compare unequal (never panic).
        assert!(!verify_hash(PEPPER, "s", "tooshort"));
        assert!(!verify_hash(PEPPER, "s", &"z".repeat(64)));
        // The real hash verifies.
        assert!(verify_hash(PEPPER, "s", &h));
    }

    #[test]
    fn hashes_are_deterministic_and_hex_shaped() {
        let h = key_hash(PEPPER, "abc");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(key_hash(PEPPER, "abc"), h);
    }
}
