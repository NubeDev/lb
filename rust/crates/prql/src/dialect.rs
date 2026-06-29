//! The dialect map — the small set of SQL dialects the query surface compiles to, and how the host
//! picks one from a query `target` or a datasource `kind`. Kept narrow on purpose: a new dialect is a
//! deliberate addition (a golden test + a `kind` mapping). The platform target compiles to `Generic`
//! (standard SQL), which is the relational subset SurrealDB's `store.query` parse-allowlist accepts.

use prqlc::sql::Dialect as PrqlDialect;
use prqlc::{Options, Target};

use super::error::PrqlError;

/// The dialects a saved query may compile to. `Generic` is the platform target's dialect (standard
/// SQL that SurrealDB's read-only gate accepts as its relational subset); the others are picked from
/// a datasource's `kind` for the `federation.query` path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// Standard SQL — the platform target (run through `store.query`'s parse-allowlist).
    Generic,
    /// A registered Postgres datasource (`federation.query`).
    Postgres,
    /// A registered MySQL datasource (`federation.query`).
    MySql,
    /// A registered DuckDB datasource (`federation.query`).
    DuckDb,
}

impl Dialect {
    /// The `prqlc` compile options for this dialect (no signature comment — clean SQL only).
    pub(crate) fn options(self) -> Options {
        let d = match self {
            Dialect::Generic => PrqlDialect::Generic,
            Dialect::Postgres => PrqlDialect::Postgres,
            Dialect::MySql => PrqlDialect::MySql,
            Dialect::DuckDb => PrqlDialect::DuckDb,
        };
        Options::default()
            .with_target(Target::Sql(Some(d)))
            .no_signature()
    }
}

/// Pick a dialect from a query `target` string. `"platform"` → `Generic`; `"datasource:<name>"` is
/// caller-resolved to its `kind` first (see [`dialect_for_kind`]); anything else is a bad target.
pub fn dialect_for_target(target: &str) -> Result<Dialect, PrqlError> {
    if target == "platform" {
        return Ok(Dialect::Generic);
    }
    Err(PrqlError::BadDialect(format!(
        "target `{target}` resolves no dialect — expected `platform` or `datasource:<name>`"
    )))
}

/// Map a registered datasource's `kind` to its PRQL dialect. Unknown kinds (no PRQL mapping) error —
/// the author gets a clear message rather than a silent wrong-dialect compile.
pub fn dialect_for_kind(kind: &str) -> Result<Dialect, PrqlError> {
    match kind {
        "postgres" | "timescale" => Ok(Dialect::Postgres),
        "mysql" => Ok(Dialect::MySql),
        "duckdb" => Ok(Dialect::DuckDb),
        other => Err(PrqlError::BadDialect(format!(
            "datasource kind `{other}` maps to no prql dialect"
        ))),
    }
}
