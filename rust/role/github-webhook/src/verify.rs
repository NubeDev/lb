//! Verify a GitHub webhook delivery's `X-Hub-Signature-256` header: `HMAC-SHA256(secret, body)`,
//! hex-encoded, prefixed `sha256=`. This is the *transport authenticity* gate — proof the delivery
//! really came from someone holding the shared secret (GitHub) and that the body was not altered in
//! transit. It is layered BEFORE the host's capability gates, never instead of them.
//!
//! Two correctness points the GitHub docs are explicit about, both honoured here:
//!   1. **Compare in constant time.** A short-circuiting `==` on the hex digests leaks, byte by
//!      byte, how much of a forged signature is correct — a timing oracle that lets an attacker
//!      recover a valid MAC. We compare the raw 32-byte MACs with an XOR-accumulate that always
//!      touches every byte.
//!   2. **Verify over the EXACT raw bytes.** The MAC is over the body GitHub sent; re-serializing
//!      parsed JSON would change bytes (key order, whitespace) and never match. The route hands us
//!      the raw body and we hash that, before any parse.
//!
//! The secret never appears in an error: every failure is the same opaque [`SignatureError`], and
//! the route maps it to a bare `401` with no detail (no "expected X got Y" oracle, no secret leak).

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Why a delivery's signature did not verify. Deliberately coarse — a caller (and the wire) learns
/// only "not authentic", never *which* check failed or what the expected value was.
#[derive(Debug, PartialEq, Eq)]
pub enum SignatureError {
    /// The `X-Hub-Signature-256` header was absent (an unsigned delivery — reject; the secret is
    /// configured, so every legitimate delivery is signed).
    Missing,
    /// The header was present but not the expected `sha256=<64 hex chars>` shape.
    Malformed,
    /// The header was well-formed but the MAC did not match the body under the secret (a forgery, a
    /// tampered body, or the wrong secret — indistinguishable on purpose).
    Mismatch,
}

/// Verify `signature_header` (the `X-Hub-Signature-256` value, e.g. `sha256=abcd…`) against the raw
/// request `body` under `secret`. `Ok(())` iff the delivery is authentic.
pub fn verify_signature(
    secret: &[u8],
    body: &[u8],
    signature_header: Option<&str>,
) -> Result<(), SignatureError> {
    let header = signature_header.ok_or(SignatureError::Missing)?;
    let hex = header
        .strip_prefix("sha256=")
        .ok_or(SignatureError::Malformed)?;
    let provided = decode_hex32(hex).ok_or(SignatureError::Malformed)?;

    // HMAC keys are arbitrary-length; `new_from_slice` never fails for HMAC, but handle it as a
    // mismatch rather than unwrap (an empty/garbage secret must not panic the front door).
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| SignatureError::Mismatch)?;
    mac.update(body);
    let expected = mac.finalize().into_bytes();

    if constant_time_eq(&provided, expected.as_slice()) {
        Ok(())
    } else {
        Err(SignatureError::Mismatch)
    }
}

/// Decode exactly 64 lowercase/uppercase hex chars into 32 bytes. `None` on any non-hex char or a
/// wrong length — a malformed signature, indistinguishable from a forgery to the caller.
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

/// Constant-time byte-slice equality: always touches every byte of `a`, never short-circuits, so
/// the time taken does not reveal how many leading bytes matched. Returns `false` immediately only
/// on a length mismatch (lengths are not secret here — both are a fixed 32).
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

#[cfg(test)]
mod tests {
    use super::*;

    /// HMAC-SHA256 of `body` under `secret`, as the `sha256=<hex>` header GitHub sends. The test's
    /// own signer, so the verifier is checked against an independent computation of the MAC.
    fn sign(secret: &[u8], body: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(secret).unwrap();
        mac.update(body);
        let mac = mac.finalize().into_bytes();
        let mut hex = String::with_capacity(64);
        for b in mac {
            hex.push(char::from_digit((b >> 4) as u32, 16).unwrap());
            hex.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
        }
        format!("sha256={hex}")
    }

    #[test]
    fn a_correct_signature_verifies() {
        let secret = b"s3cr3t";
        let body = br#"{"action":"opened"}"#;
        let sig = sign(secret, body);
        assert_eq!(verify_signature(secret, body, Some(&sig)), Ok(()));
    }

    #[test]
    fn a_tampered_body_does_not_verify() {
        let secret = b"s3cr3t";
        let sig = sign(secret, br#"{"action":"opened"}"#);
        // Same signature, body altered after signing — the MAC no longer matches.
        assert_eq!(
            verify_signature(secret, br#"{"action":"closed"}"#, Some(&sig)),
            Err(SignatureError::Mismatch)
        );
    }

    #[test]
    fn the_wrong_secret_does_not_verify() {
        let body = br#"{"action":"opened"}"#;
        let sig = sign(b"the-real-secret", body);
        assert_eq!(
            verify_signature(b"a-different-secret", body, Some(&sig)),
            Err(SignatureError::Mismatch)
        );
    }

    #[test]
    fn a_missing_header_is_missing_not_a_pass() {
        assert_eq!(
            verify_signature(b"s", b"body", None),
            Err(SignatureError::Missing)
        );
    }

    #[test]
    fn malformed_headers_are_rejected() {
        let secret = b"s";
        // No `sha256=` prefix.
        assert_eq!(
            verify_signature(secret, b"b", Some("deadbeef")),
            Err(SignatureError::Malformed)
        );
        // Right prefix, wrong length.
        assert_eq!(
            verify_signature(secret, b"b", Some("sha256=abcd")),
            Err(SignatureError::Malformed)
        );
        // Right length, non-hex char.
        let bad = format!("sha256={}", "z".repeat(64));
        assert_eq!(
            verify_signature(secret, b"b", Some(&bad)),
            Err(SignatureError::Malformed)
        );
    }
}
