//! The DB-browser service error. A `Denied` carries no detail — an un-granted (or non-admin) caller
//! leaks nothing about what tables/records exist (data-console scope, the gate-3-relaxation risk).
//! Mirrors `IngestError`/`AssetError`.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbViewError {
    /// Authorization failed (workspace isolation or the missing admin capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The read targets the **secret plane** (`lb_store::SECRET_TABLES`) — the credential tables the
    /// owner gate on `secret.get` protects. Refused for every principal including a workspace admin
    /// and the host, with no override capability, on every raw-read surface. Unlike `Denied` this one
    /// names the table: the caller supplied it, so the message carries no existence signal, and a
    /// generic error would read as a bug rather than a rule.
    #[error("rejected: reading the secret-plane table '{0}' is not allowed on any read surface")]
    SecretTable(&'static str),
    /// The durable store rejected the read.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
