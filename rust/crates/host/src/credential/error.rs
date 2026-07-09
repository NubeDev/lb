//! The credential service error. `Denied` is opaque (§3.5). `BadInput` guards an empty secret at the
//! write verb. `Store` is the durable-store failure. `Hash` wraps an argon2 hashing failure. No
//! variant ever carries the plaintext or the hash (secrets rule §6.7).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CredentialError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// A caller-facing input problem (e.g. an empty secret). Never echoes the secret.
    #[error("bad input: {0}")]
    BadInput(String),
    /// An internal argon2 hashing/verify failure (never a wrong-password — that is a `false`, not an
    /// error). Message is the library's, carries no secret material.
    #[error("hash error: {0}")]
    Hash(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
