//! The compact JWS codec (`base64url(header).base64url(payload).base64url(sig)`), signed with
//! Ed25519 directly. We own the whole token with one crypto library — no JWT/ring seam (see
//! debugging/auth/valid-token-fails-verification.md).
//!
//! The header is fixed (`{"alg":"EdDSA","typ":"JWT"}`); the payload is the [`Claims`] JSON.

use base64ct::{Base64UrlUnpadded, Encoding};

const HEADER_JSON: &str = r#"{"alg":"EdDSA","typ":"JWT"}"#;

/// The signing input for a serialized claims payload: `b64(header).b64(payload)`.
pub(crate) fn signing_input(payload_json: &[u8]) -> String {
    let header = Base64UrlUnpadded::encode_string(HEADER_JSON.as_bytes());
    let payload = Base64UrlUnpadded::encode_string(payload_json);
    format!("{header}.{payload}")
}

/// Assemble the full compact token from a signing input and the raw signature bytes.
pub(crate) fn assemble(signing_input: &str, sig: &[u8]) -> String {
    let sig_b64 = Base64UrlUnpadded::encode_string(sig);
    format!("{signing_input}.{sig_b64}")
}

/// Split a compact token into (signing_input, payload_bytes, signature_bytes). `None` if the
/// shape is wrong or any segment is not valid base64url.
pub(crate) fn split(token: &str) -> Option<(String, Vec<u8>, Vec<u8>)> {
    let mut parts = token.split('.');
    let header = parts.next()?;
    let payload = parts.next()?;
    let sig = parts.next()?;
    if parts.next().is_some() {
        return None; // too many segments
    }
    let payload_bytes = Base64UrlUnpadded::decode_vec(payload).ok()?;
    let sig_bytes = Base64UrlUnpadded::decode_vec(sig).ok()?;
    Some((format!("{header}.{payload}"), payload_bytes, sig_bytes))
}
