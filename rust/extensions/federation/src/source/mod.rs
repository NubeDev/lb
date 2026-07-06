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

/// A discovered table in an external source (the `federation.schema` list result).
#[derive(Debug, Clone)]
pub struct TableMeta {
    pub name: String,
    /// A row-count estimate when the source exposes one (Postgres `reltuples`); `None` otherwise.
    pub rows: Option<i64>,
}

/// A discovered column on a table (the `federation.schema` describe result).
#[derive(Debug, Clone)]
pub struct ColumnMeta {
    pub name: String,
    /// The source's reported type, rendered as a string (e.g. `integer`, `text`).
    pub data_type: String,
    pub nullable: bool,
}

/// A discovered foreign key on a table (the `federation.sample` relationships result): `column`
/// on the owning table references `ref_table.ref_column`.
#[derive(Debug, Clone)]
pub struct ForeignKeyMeta {
    pub column: String,
    pub ref_table: String,
    pub ref_column: String,
}

/// A connected external SQL source. The pool lives inside the impl; the orchestrator asks only for a
/// `TableProvider` per referenced table name (the validator collected them), then runs the query
/// through a DataFusion `SessionContext`. Discovery (`list_tables`/`describe_table`) reuses the same
/// provider path — it never reaches for an `information_schema` the engine doesn't expose.
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

    /// List the user tables in the source (per-impl: each knows its own catalog query). Used by the
    /// `federation.schema` discovery verb so a non-SQL UI can browse without writing a query.
    async fn list_tables(&self) -> Result<Vec<TableMeta>, SourceError>;

    /// List `table`'s foreign keys, **best-effort** (the `federation.sample` relationships read).
    /// A kind that cannot answer returns `Ok(vec![])` — never an error: a missing FK catalog must
    /// not fail a snapshot (the AI can still infer joins from column names, like the ERD does).
    async fn foreign_keys(&self, _table: &str) -> Result<Vec<ForeignKeyMeta>, SourceError> {
        Ok(Vec::new())
    }
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
