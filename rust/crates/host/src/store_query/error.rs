//! The read-only SQL surface error. `Denied` is opaque (which gate failed — workspace, capability —
//! leaks nothing; mirrors `DbViewError`/`DashboardError`). `Rejected` carries WHY a statement was
//! refused by the parse-allowlist (a write kind, a multi-statement, a namespace-naming statement) —
//! that is author feedback for the SQL editor, not an authorization signal, so it is safe to surface.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreQueryError {
    /// Authorization failed — workspace isolation or a missing `mcp:store.query|schema:call`. Opaque
    /// by design (no detail on which gate, no existence signal).
    #[error("denied")]
    Denied,
    /// The statement did not pass the read-only parse-allowlist: not a single `SELECT`/`INFO`/`SHOW`,
    /// or it named a namespace/database (a `USE`, or a `DEFINE NAMESPACE`-class statement). This is
    /// the load-bearing read-only gate — surfaced so the SQL editor can show the author what's wrong.
    #[error("rejected: {0}")]
    Rejected(String),
    /// The statement reads (or could read) the **secret plane** — the credential tables the owner
    /// gate on `secret.get` protects. Refused for every principal, with no override capability, on
    /// every read surface (`secret_wall.rs`). Named, not opaque: the caller wrote the table, so the
    /// message carries no existence signal, and a generic 500 would just look like a bug.
    #[error("rejected: reading the secret-plane table '{0}' is not allowed on any read surface")]
    SecretTable(&'static str),
    /// The statement was syntactically invalid SurrealQL (the parser refused it before we could even
    /// allowlist its kind).
    #[error("parse error: {0}")]
    Parse(String),
    /// The durable store rejected the operation (a runtime fault, a timeout).
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
