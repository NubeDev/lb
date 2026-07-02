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

/// Decode a token's `Claims` **without verifying the signature** — a client-side introspection read
/// (the operator CLI's `whoami`/header renders `ws`/`sub`/`role` from the token it already holds). This
/// is NOT an authorization path and grants nothing: the server re-verifies every request with the node
/// key, so a forged/edited payload only mis-labels the caller's own terminal, never widens access.
/// `None` if the token shape or payload JSON is malformed.
pub fn claims_unverified(token: &str) -> Option<crate::claims::Claims> {
    let (_signing_input, payload, _sig) = split(token)?;
    serde_json::from_slice(&payload).ok()
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
