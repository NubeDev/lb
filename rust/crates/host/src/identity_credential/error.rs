//! The global-credential service error (email-login scope). `Denied` is opaque (§3.5). `BadInput`
//! guards an empty secret. `BadOldSecret` is the self-service change's wrong-current-password case
//! (a `401` at the route, never revealing whether the identity or the password was the problem).
//! `Store`/`Hash` are the durable-store / argon2 failures. No variant ever carries the plaintext or
//! the hash (§6.7).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityCredentialError {
    /// Authorization failed (missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// A caller-facing input problem (e.g. an empty secret). Never echoes the secret.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The self-service change presented the wrong current password (or none is set). `401`.
    #[error("bad current credential")]
    BadOldSecret,
    /// An internal argon2 hashing/verify failure. Message carries no secret material.
    #[error("hash error: {0}")]
    Hash(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
