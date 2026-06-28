//! The bus pub/sub service error (widget-config-vars scope, "Platform fix"). A `Denied` is opaque (no
//! gate detail) — an un-granted caller learns nothing. `BadSubject` covers a reserved-prefix / malformed
//! subject (the workspace-wall guard). Mirrors `IngestError`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BusError {
    /// Authorization failed (workspace isolation or missing `mcp:bus.publish|watch:call`). Opaque.
    #[error("denied")]
    Denied,
    /// The subject is empty, names a reserved prefix, or tries to escape the workspace wall.
    #[error("bad subject: {0}")]
    BadSubject(String),
    /// The payload / arguments were not valid for the verb.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The underlying bus transport failed.
    #[error("bus error: {0}")]
    Bus(String),
}
