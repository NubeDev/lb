//! Authenticate a request: read the bearer token and verify it with the node key.
//!
//! This is the gateway's identity chokepoint. A missing/garbled header is `401` (unauthenticated);
//! a forged or expired token is `401` (`lb_auth::verify` rejects it). On success the returned
//! [`Principal`] carries the workspace + caps from the *token* — so a route can never read the
//! workspace from the request body and the workspace wall (§7) holds at the front door.

use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use lb_auth::{verify, Principal};

use crate::state::Gateway;

/// Why authentication failed — both collapse to `401` so a caller can't distinguish "no token"
/// from "bad token" (no oracle), but kept distinct in the message for dev ergonomics.
#[derive(Debug)]
pub enum AuthRejection {
    /// No `Authorization: Bearer` header, or it was malformed.
    Missing,
    /// The token failed signature or expiry verification.
    Invalid,
}

impl AuthRejection {
    /// Map to the HTTP response a route returns. Always `401` — authenticity is decided before
    /// authority (a `403` would leak that the token was valid but ungranted).
    pub fn into_response(self) -> (StatusCode, String) {
        let msg = match self {
            AuthRejection::Missing => "missing bearer token",
            AuthRejection::Invalid => "invalid or expired token",
        };
        (StatusCode::UNAUTHORIZED, msg.to_string())
    }
}

/// Verify the request's bearer token against the node key at the gateway's clock, returning the
/// verified principal. The single entry every guarded route uses to learn "who is calling".
pub fn authenticate(gw: &Gateway, headers: &HeaderMap) -> Result<Principal, AuthRejection> {
    let raw = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AuthRejection::Missing)?;
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))
        .ok_or(AuthRejection::Missing)?
        .trim();
    verify_token(gw, token)
}

/// Verify a bare token string (no header framing) — the SSE path. `EventSource` cannot set an
/// `Authorization` header, so the stream route carries the token as a `?token=` query param and
/// hands the raw value here. The verification is identical (same key, same clock, same `verify`).
pub fn verify_token(gw: &Gateway, token: &str) -> Result<Principal, AuthRejection> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AuthRejection::Missing);
    }
    verify(&gw.key, token, gw.now).map_err(|_| AuthRejection::Invalid)
}
