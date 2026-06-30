//! The typed error for PRQL compilation. Maps onto the host `query` service's error at the bridge:
//! a compile failure is AUTHOR FEEDBACK (a malformed query), surfaced as `BadInput` — never an auth
//! signal. A bad target/dialect is also author feedback.

use thiserror::Error;

/// A PRQL compile / dialect error. Never an authorization signal.
#[derive(Debug, Error)]
pub enum PrqlError {
    /// The PRQL text failed to parse or compile. Carries `prqlc`'s human-readable message (the
    /// editor surfaces it verbatim in the compile preview).
    #[error("prql compile error: {0}")]
    Compile(String),
    /// The requested target / datasource kind maps to no PRQL dialect we support.
    #[error("unsupported target/dialect: {0}")]
    BadDialect(String),
}
