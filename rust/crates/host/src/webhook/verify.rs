//! `signature`-mode HMAC verification over the **raw request body** (webhooks scope). v1 ships the
//! `hmac-sha256` scheme only (open question, resolved): the caller signs the raw bytes with the
//! shared secret and sends the result in an admin-picked header, value `sha256=<64 lowercase hex>`.
//!
//! Two correctness invariants pinned by tests:
//!   1. **Compare in constant time.** A short-circuiting `==` on hex digests leaks how many
//!      leading bytes of a forged MAC are correct — a timing oracle that can recover a valid MAC.
//!      We decode the hex and compare the 32-byte MACs with an XOR-accumulate that always touches
//!      every byte (the same discipline `lb_apikey::hash` and the github-webhook verifier use).
//!   2. **Verify over the EXACT received bytes.** Re-serializing parsed JSON changes bytes (key
//!      order, whitespace) and never matches a real signature. The gateway route MUST capture the
//!      raw body before any JSON parse and hand those bytes here — pinned by a body-tamper test.
//!
//! The secret never appears in an error: every failure is the same opaque [`SignatureError`], and
//! the route maps it to a bare `401` (no "expected X got Y" oracle, no secret leak). This mirrors
//! `lb_role_github_webhook::verify` — the discipline is vetted; this is the generic form.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Why a `signature`-mode delivery did not verify. Deliberately coarse — a caller learns only "not
/// authentic", never *which* check failed or what the expected value was (no oracle).
#[derive(Debug, PartialEq, Eq)]
pub enum SignatureError {
    /// The configured header was absent from the request (an unsigned delivery — reject; the
    /// secret is configured, so every legitimate delivery is signed).
    Missing,
    /// The header was present but not the expected `sha256=<64 hex chars>` shape.
    Malformed,
    /// The header was well-formed but the MAC did not match the body under the secret (a forgery,
    /// a tampered body, or the wrong secret — indistinguishable on purpose).
    Mismatch,
}

/// Verify `header_value` (e.g. `sha256=abcd…`) against the raw request `body` under `secret`.
/// `Ok(())` iff the delivery is authentic. The scheme is fixed `hmac-sha256` (v1).
pub fn verify_signature(
    secret: &[u8],
    body: &[u8],
    header_value: Option<&str>,
) -> Result<(), SignatureError> {
    let header = header_value.ok_or(SignatureError::Missing)?;
    let hex = header
        .strip_prefix("sha256=")
        .ok_or(SignatureError::Malformed)?;
    let provided = decode_hex32(hex).ok_or(SignatureError::Malformed)?;

    // HMAC keys are arbitrary-length; `new_from_slice` never fails for HMAC, but treat an
    // empty/garbage secret as a mismatch rather than unwrap (must never panic the front door).
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| SignatureError::Mismatch)?;
    mac.update(body);
    let expected = mac.finalize().into_bytes();

    if constant_time_eq(&provided, expected.as_slice()) {
        Ok(())
    } else {
        Err(SignatureError::Mismatch)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// HMAC-SHA256 of `body` under `secret`, as the `sha256=<hex>` header the universal shape
    /// sends. The test's own signer, so the verifier is checked against an independent computation.
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
        let body = br#"{"event":"ping"}"#;
        let sig = sign(secret, body);
        assert_eq!(verify_signature(secret, body, Some(&sig)), Ok(()));
    }

    #[test]
    fn a_tampered_body_does_not_verify() {
        let secret = b"s3cr3t";
        let sig = sign(secret, br#"{"event":"ping"}"#);
        // Same signature, body altered after signing — the MAC no longer matches (the raw-bytes
        // invariant: any middleware that re-serializes JSON would break every real signature).
        assert_eq!(
            verify_signature(secret, br#"{"event":"pong"}"#, Some(&sig)),
            Err(SignatureError::Mismatch)
        );
    }

    #[test]
    fn the_wrong_secret_does_not_verify() {
        let body = br#"{"event":"ping"}"#;
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

    #[test]
    fn whitespace_signatures_do_not_validate() {
        let secret = b"s";
        let body = b"[]";
        // A leading space breaks the `sha256=` prefix strip → Malformed.
        assert_eq!(
            verify_signature(secret, body, Some(" sha256=abc")),
            Err(SignatureError::Malformed)
        );
    }
}
