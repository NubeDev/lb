//! The `Source` trait — the ONE sanctioned fake-boundary (testing-scope §0): a true external SQL
//! engine reached behind a single trait, one impl file per kind. The DSN comes from the host's
//! secret-mediation (handed in the call input, never hardcoded, never logged); the impl owns the
//! connection pool. Everything above this trait (validate, query orchestration) is engine-agnostic.
//!
//! Two kinds ship in v1: `postgres` (the headline — Postgres/Timescale via a real pool) and `sqlite`
//! (a real on-disk SQLite engine, the documented test fallback when Docker is unavailable). Both are
//! REAL external engines behind the trait — never an in-process re-implementation of a database.
//!
//! Attribution: the embedded-DataFusion + per-table-provider registration pattern is adapted from
//! `rubix-cube` (its `spice_engine` wrapper over the `datafusion` crate), MIT/Apache-2.0.

#[cfg(feature = "postgres")]
mod postgres;
mod sqlite;

use std::sync::Arc;

use datafusion::catalog::TableProvider;
use datafusion::sql::TableReference;

#[cfg(feature = "postgres")]
pub use postgres::PostgresSource;
pub use sqlite::SqliteSource;

/// A connected external SQL source. The pool lives inside the impl; the orchestrator asks only for a
/// `TableProvider` per referenced table name (the validator collected them), then runs the query
/// through a DataFusion `SessionContext`.
#[async_trait::async_trait]
pub trait Source: Send + Sync {
    /// A real connectivity probe — open the pool and run a trivial query. `Ok(())` is green.
    async fn probe(&self) -> Result<(), SourceError>;

    /// Build a DataFusion `TableProvider` for `table` against the live pool. The provider pushes the
    /// query down to the remote engine (the whole point of federation).
    async fn table_provider(
        &self,
        table: &TableReference,
    ) -> Result<Arc<dyn TableProvider>, SourceError>;
}

/// A source-layer error. The DSN is NEVER included in the message (secret mediation — datasources
/// scope: the connection string never reaches a log or a result).
#[derive(Debug)]
pub struct SourceError(pub String);

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source error: {}", self.0)
    }
}

impl std::error::Error for SourceError {}

/// Construct a `Source` for `kind` from its `dsn`. The DSN is consumed here and lives only inside the
/// pool — it is not retained anywhere a log/result could observe it.
pub async fn connect(kind: &str, dsn: &str) -> Result<Box<dyn Source>, SourceError> {
    match kind {
        #[cfg(feature = "postgres")]
        "postgres" | "timescale" => Ok(Box::new(PostgresSource::connect(dsn).await?)),
        #[cfg(not(feature = "postgres"))]
        "postgres" | "timescale" => Err(SourceError(
            "postgres source not built in (rebuild federation with --features postgres)".into(),
        )),
        "sqlite" => Ok(Box::new(SqliteSource::connect(dsn).await?)),
        other => Err(SourceError(format!("unknown source kind: {other}"))),
    }
}
