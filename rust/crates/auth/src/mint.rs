//! Mint a signed token from a claim set (hub-side). The clock is the caller's — pass explicit
//! `iat`/`exp` in `Claims` so tests are deterministic (testing §3).

use crate::claims::Claims;
use crate::keypair::SigningKey;
use crate::token::{assemble, signing_input};

/// Serialize `claims`, build the JWS signing input, sign it with the node key, and return the
/// compact token string.
pub fn mint(key: &SigningKey, claims: &Claims) -> String {
    let payload = serde_json::to_vec(claims).expect("claims always serialize");
    let input = signing_input(&payload);
    let sig = key.sign(input.as_bytes());
    assemble(&input, &sig.to_bytes())
}
