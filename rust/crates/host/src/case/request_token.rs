//! The **request token** — mint one, hash one (case-plane scope, wave 2).
//!
//! The token IS the contractor's identity. There is no account, no password and no session behind
//! it, so it carries the entire weight of the round trip, and three properties are non-negotiable:
//!
//! 1. **≥256 bits of cryptographic randomness.** `lb_apikey::generate_secret` is the node's one
//!    CSPRNG-backed secret mint (32 random bytes, Crockford base32) — the same source the invite
//!    token and every API key use. Nothing here rolls its own.
//! 2. **Only the SHA-256 hash is stored.** The raw token exists exactly once, in the link in the
//!    email. A full-entropy random token makes a fast hash correct (it is not a user-chosen
//!    password, so there is nothing to brute-force and no reason for a KDF) — the invite token's
//!    reasoning, unchanged.
//! 3. **A distinct prefix.** `lbr_` (r for *request*), so a token in a log or a bug report is
//!    identifiable as a case-request link and can never be confused with an invite (`lbi_`) or an
//!    API key (`lbk_`) by a human or by a route that matches on shape.
//!
//! **The token names its own workspace**: `lbr_{ws}.{secret}`. The public route has no session and
//! no path segment to learn the tenancy from — an invite accept is told the workspace in its body,
//! and a webhook has it in its URL, but a contractor's link is a bare string in an email. Scanning
//! every workspace for a matching hash would be both a cross-tenant read and an unbounded one. So
//! the workspace rides in front, and because the hash is taken over the WHOLE string it is bound to
//! it: edit the workspace half and the token resolves to nothing at all.
//!
//! A deliberate near-copy of `invites/token.rs` rather than a shared helper: the two are the same
//! ten lines today by coincidence of both being "a random bearer string", and folding them together
//! would make an invite's token format a dependency of the contractor plane. If either ever needs a
//! different length or encoding, it changes alone.

use lb_apikey::generate_secret;
use sha2::{Digest, Sha256};

/// The case-request token prefix.
pub const TOKEN_PREFIX: &str = "lbr_";

/// The separator between the workspace and the secret. A `.` because a workspace id is a slug and
/// cannot contain one — [`workspace_of_token`] refuses anything else rather than guessing.
const WS_SEPARATOR: char = '.';

/// Mint a fresh request token for `ws`. Shown exactly once, in the link.
pub(super) fn generate_request_token(ws: &str) -> String {
    format!("{TOKEN_PREFIX}{ws}{WS_SEPARATOR}{}", generate_secret())
}

/// The workspace a presented token names, or `None` when it is not a request token at all.
///
/// Parsing only — it asserts nothing about the token being real. The hash lookup is what decides
/// that, and it runs against this workspace's rows only, so a token naming a workspace that does
/// not exist resolves to nothing exactly like a token naming one that does but has no such request.
pub fn workspace_of_token(token: &str) -> Option<&str> {
    let rest = token.strip_prefix(TOKEN_PREFIX)?;
    let (ws, secret) = rest.split_once(WS_SEPARATOR)?;
    (!ws.is_empty() && !secret.is_empty()).then_some(ws)
}

/// SHA-256 of the raw token, lowercase hex. Stored as `case_request.token_hash`.
pub fn hash_request_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap_or('0'));
        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap_or('0'));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_prefixed_unique_and_long_enough_to_be_unguessable() {
        let a = generate_request_token("nube");
        let b = generate_request_token("nube");
        assert!(a.starts_with(TOKEN_PREFIX));
        assert_ne!(a, b, "two mints must never collide");
        // 32 random bytes in Crockford base32 ⇒ 52 characters after the prefix. The assertion is on
        // the LENGTH rather than the entropy source because a future change that shortened the
        // secret would silently weaken every outstanding link.
        assert!(
            a.len() - TOKEN_PREFIX.len() - "nube.".len() >= 51,
            "a request token must carry at least 256 bits: {a}"
        );
    }

    #[test]
    fn the_workspace_rides_in_front_and_a_malformed_token_names_none() {
        assert_eq!(
            workspace_of_token(&generate_request_token("nube")),
            Some("nube")
        );
        assert_eq!(
            workspace_of_token("lbi_someinvite"),
            None,
            "an invite is not a request token"
        );
        assert_eq!(workspace_of_token("lbr_nube"), None, "no secret half");
        assert_eq!(workspace_of_token("lbr_.secret"), None, "no workspace half");
        assert_eq!(workspace_of_token("nonsense"), None);
    }

    #[test]
    fn a_tampered_workspace_changes_the_hash() {
        // The whole string is hashed, so the workspace half cannot be swapped for another
        // tenant's without invalidating the token.
        let token = generate_request_token("nube");
        let elsewhere = token.replacen("nube", "acme", 1);
        assert_ne!(hash_request_token(&token), hash_request_token(&elsewhere));
    }

    #[test]
    fn the_hash_is_deterministic_hex_and_reveals_nothing_of_the_token() {
        let token = generate_request_token("nube");
        let hash = hash_request_token(&token);
        assert_eq!(hash, hash_request_token(&token));
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!hash.contains(&token[TOKEN_PREFIX.len()..]));
    }
}
