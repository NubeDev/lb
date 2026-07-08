//! Verify a token offline with the node key and produce a `Principal`.
//!
//! Proves the Ed25519 signature, then that the token is unexpired at the supplied `now`
//! (injected for determinism — testing §3). Workspace isolation and capabilities are NOT
//! checked here; that is `caps::check`'s job once a valid principal exists.

use ed25519_dalek::Signature;
use thiserror::Error;

use crate::claims::Claims;
use crate::keypair::SigningKey;
use crate::principal::Principal;
use crate::token::split;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("token signature invalid or malformed")]
    BadToken,
    #[error("token expired")]
    Expired,
}

/// Verify `token` against `key` at time `now` (unix seconds). On success, returns the
/// `Principal` the rest of the host trusts.
pub fn verify(key: &SigningKey, token: &str, now: u64) -> Result<Principal, AuthError> {
    let (signing_input, payload, sig_bytes) = split(token).ok_or(AuthError::BadToken)?;

    let sig = Signature::from_slice(&sig_bytes).map_err(|_| AuthError::BadToken)?;
    if !key.verify(signing_input.as_bytes(), &sig) {
        return Err(AuthError::BadToken);
    }

    let claims: Claims = serde_json::from_slice(&payload).map_err(|_| AuthError::BadToken)?;
    if now >= claims.exp {
        return Err(AuthError::Expired);
    }

    // A run-scoped token (agent-key-lifecycle D1–D5) carries `constraint` (the caller's caps, so
    // gate 2b fires on the verified principal) + `run_id` (so the gateway can refuse a terminal
    // run's token). An ordinary token has neither — `Principal::from_token_claims` with two
    // `None`s is equivalent to the old `Principal::new`.
    Ok(Principal::from_token_claims(
        claims.sub,
        claims.ws,
        claims.role,
        claims.caps,
        claims.constraint,
        claims.run_id,
    ))
}
