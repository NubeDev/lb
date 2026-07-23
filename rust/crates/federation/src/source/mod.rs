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

use arrow::record_batch::RecordBatch;
use datafusion::catalog::TableProvider;
use datafusion::sql::TableReference;
use serde_json::Value;

#[cfg(feature = "postgres")]
pub use postgres::PostgresSource;
pub use sqlite::SqliteSource;

pub mod dialect;
#[allow(unused_imports)]
pub use dialect::{
    canonicalize_live_type, plan_migrate, DdlStatement, DesignColumn, DesignFk, DesignSchema,
    DesignTable, DestructiveRefusal, LiveCatalog, LiveColumn, NEUTRAL_TYPES,
};

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

    /// Execute raw SQL directly against the source, bypassing the DataFusion planning/unparsing
    /// ceremony. Returns Arrow record batches. Only safe for single-table SELECTs where the
    /// application layer has already validated the query shape — no JOINs, subqueries, CTEs,
    /// or set operations that would require multi-source orchestration.
    ///
    /// The default impl returns an error; sources opt in by implementing against their pool.
    async fn query_direct(&self, _sql: &str) -> Result<Vec<RecordBatch>, SourceError> {
        Err(SourceError("direct query not supported by this source".into()))
    }

    /// Execute raw SQL and return JSON values directly, skipping the Arrow intermediate layer
    /// entirely. Returns `(column_names, rows)` where each row is a column-aligned JSON array.
    ///
    /// The default impl converts through `query_direct` (which goes through Arrow) for sources
    /// that don't implement this directly. Sources that CAN skip Arrow (e.g. Postgres) SHOULD
    /// override this to eliminate the ~500ms Postgres→Arrow→JSON double-conversion overhead.
    async fn query_direct_json(&self, sql: &str) -> Result<(Vec<String>, Vec<Value>), SourceError> {
        let batches = self.query_direct(sql).await?;
        let columns: Vec<String> = match batches.first() {
            Some(b) => b
                .schema()
                .fields()
                .iter()
                .map(|f| f.name().clone())
                .collect(),
            None => return Ok((Vec::new(), Vec::new())),
        };

        // Arrow → JSON via the same path as `query::shape` (arrow_json::ArrayWriter).
        let mut buf = Vec::new();
        {
            let mut writer = arrow_json::ArrayWriter::new(&mut buf);
            for batch in &batches {
                writer
                    .write(batch)
                    .map_err(|e| SourceError(format!("arrow->json: {e}")))?;
            }
            writer
                .finish()
                .map_err(|e| SourceError(format!("arrow->json finish: {e}")))?;
        }

        let objs: Vec<Value> = if buf.is_empty() {
            Vec::new()
        } else {
            serde_json::from_slice(&buf)
                .map_err(|e| SourceError(format!("json parse: {e}")))?
        };

        let rows: Vec<Value> = objs
            .into_iter()
            .map(|obj| {
                Value::Array(
                    columns
                        .iter()
                        .map(|c| obj.get(c).cloned().unwrap_or(Value::Null))
                        .collect(),
                )
            })
            .collect();

        Ok((columns, rows))
    }

    /// List the user tables in the source (per-impl: each knows its own catalog query). Used by the
    /// `federation.schema` discovery verb so a non-SQL UI can browse without writing a query.
    async fn list_tables(&self) -> Result<Vec<TableMeta>, SourceError>;

    /// List `table`'s foreign keys, **best-effort** (the `federation.sample` relationships read).
    /// A kind that cannot answer returns `Ok(vec![])` — never an error: a missing FK catalog must
    /// not fail a snapshot (the AI can still infer joins from column names, like the ERD does).
    async fn foreign_keys(&self, _table: &str) -> Result<Vec<ForeignKeyMeta>, SourceError> {
        Ok(Vec::new())
    }

    /// List `table`'s columns with their types NORMALIZED to the canonical vocabulary (the
    /// migrate-diff read). This is the load-bearing function for diff idempotence — `varchar(255)`
    /// and `text` must both read as `text`, or migrate plans spurious ALTERs forever (scope Risk 1).
    /// Default impl derives this from the Arrow schema; per-kind impls may override with a direct
    /// catalog read (sqlite `PRAGMA table_info`, postgres `information_schema.columns`) for richer
    /// type names than Arrow exposes.
    async fn list_columns_with_types(
        &self,
        table: &str,
        kind: &str,
    ) -> Result<Vec<LiveColumn>, SourceError> {
        let provider = self.table_provider(&TableReference::bare(table)).await?;
        let schema = provider.schema();
        Ok(schema
            .fields()
            .iter()
            .map(|f| LiveColumn {
                name: f.name().clone(),
                neutral_type: canonicalize_live_type(&f.data_type().to_string(), kind),
                nullable: f.is_nullable(),
            })
            .collect())
    }

    /// Apply a batch of DDL statements atomically (one transaction where the dialect allows it —
    /// Postgres DDL is transactional; sqlite wraps the batch in `BEGIN`/`COMMIT`). Used by the
    /// `federation.migrate` apply step. The statements are the planner's allow-listed output
    /// (CREATE TABLE / ADD COLUMN / ADD CONSTRAINT FK) — never caller SQL.
    async fn apply_ddl(&self, stmts: &[DdlStatement]) -> Result<(), SourceError>;

    /// Write `rows` (each a column-aligned `Vec<Value>`) into `table`. When `key` names conflict
    /// columns, the write is an UPSERT (ON CONFLICT DO UPDATE) — idempotent under redelivery
    /// (scope: a flow firing twice writes the same row once). Returns the affected row count.
    /// Used by `federation.write` and `federation.export`. Values are parameterized (never
    /// inlined into SQL) — a caller cannot inject SQL through a cell value.
    async fn write_rows(
        &self,
        table: &str,
        columns: &[String],
        rows: &[Vec<serde_json::Value>],
        key: Option<&[String]>,
    ) -> Result<u64, SourceError>;

    /// Delete every row matching a structured key from `table`. `key` names the identifying
    /// columns; each entry in `rows` is a `key`-aligned `Vec<Value>` of values for those columns,
    /// so one row here is one `DELETE ... WHERE k1=? AND k2=? ...`. All the DELETEs run in ONE
    /// transaction (a mid-batch failure rolls everything back). Returns the affected row count.
    /// Used by `federation.delete`. Values are parameterized (never inlined into SQL) — a caller
    /// cannot inject SQL through a key value, and the caller NEVER supplies SQL.
    async fn delete_rows(
        &self,
        table: &str,
        key: &[String],
        rows: &[Vec<serde_json::Value>],
    ) -> Result<u64, SourceError>;
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
///
/// Returns an `Arc` rather than a `Box` so one connected source can be **shared across calls** by
/// the warm-pool cache (`crate::pool`) — building the pool per query cost ~2,500 ms against a remote
/// Timescale (federation-pool-cache scope). Callers still reach the trait via `.as_ref()`,
/// unchanged. Prefer `pool::cached_connect` on any hot path; this is the uncached construction it
/// wraps, and the right call for a `probe` that must prove a FRESH connection works.
pub async fn connect(kind: &str, dsn: &str) -> Result<Arc<dyn Source>, SourceError> {
    match kind {
        #[cfg(feature = "postgres")]
        "postgres" | "timescale" => Ok(Arc::new(PostgresSource::connect(dsn).await?)),
        #[cfg(not(feature = "postgres"))]
        "postgres" | "timescale" => Err(SourceError(
            "postgres source not built in (rebuild federation with --features postgres)".into(),
        )),
        "sqlite" => Ok(Arc::new(SqliteSource::connect(dsn).await?)),
        other => Err(SourceError(format!("unknown source kind: {other}"))),
    }
}
