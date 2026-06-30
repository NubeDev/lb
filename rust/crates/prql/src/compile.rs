//! The single compile entry point — PRQL text → dialect-specific SQL string. Pure: no I/O. Maps a
//! `prqlc` failure to [`PrqlError::Compile`] (the author's malformed query), surfacing the compiler's
//! human-readable message verbatim for the editor preview.

use prqlc;

use super::dialect::Dialect;
use super::error::PrqlError;

/// Compile `prql` to a SQL string for `dialect`. Pure — no store, no network, no clock. A malformed
/// PRQL returns [`PrqlError::Compile`] carrying `prqlc`'s message.
pub fn compile(prql: &str, dialect: Dialect) -> Result<String, PrqlError> {
    prqlc::compile(prql, &dialect.options())
        .map_err(|e| PrqlError::Compile(e.to_string()))
        .map(|sql| sql.trim().to_string())
}
