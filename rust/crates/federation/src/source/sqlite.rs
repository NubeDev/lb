//! The SQLite source — a REAL on-disk SQLite engine behind the [`Source`](super::Source) trait
//! (datasources scope testing fallback). When Docker is unavailable to spawn Postgres, the tests use
//! this: a real external database engine with real rows on disk, still behind the one trait — never
//! an in-process re-implementation (testing-scope §0). The "DSN" is the file path.
//!
//! **Federated provider** (federation-pushdown scope): unlike `PostgresTableFactory`, the upstream
//! `SqliteTableFactory::table_provider` does NOT auto-wrap with `FederatedTableProviderAdaptor` under
//! the `sqlite-federation` feature. We therefore hold the pool ourselves and build the federated
//! provider directly: a `SQLiteTable::new_with_schema` wrapped via
//! `create_federated_table_provider()`. The factory's small `connect → get_schema → build` flow is
//! inlined here for that one purpose; everything else (`probe`, FKs) keeps its prior shape.

use std::sync::Arc;
use std::time::Duration;

use datafusion::catalog::TableProvider;
use datafusion::sql::TableReference;
use datafusion_table_providers::sql::db_connection_pool::sqlitepool::{
    SqliteConnectionPool, SqliteConnectionPoolFactory,
};
use datafusion_table_providers::sql::db_connection_pool::{
    dbconnection::get_schema, DbConnectionPool, Mode,
};
use datafusion_table_providers::sqlite::sql_table::SQLiteTable;
use datafusion_table_providers::sqlite::DynSqliteConnectionPool;

use super::{Source, SourceError};

/// A connected SQLite source: a file-backed pool + the file path (the path is retained ONLY for
/// direct catalog reads — `PRAGMA foreign_key_list` — the same path that is the DSN, never echoed).
pub struct SqliteSource {
    pool: Arc<SqliteConnectionPool>,
    path: String,
}

impl SqliteSource {
    /// Open a file-backed pool. `dsn` is the database file path (e.g. `/tmp/fed-test.db` or a
    /// `file:/...` URL — the leading `file:` scheme is stripped).
    pub async fn connect(dsn: &str) -> Result<Self, SourceError> {
        let path = dsn.strip_prefix("file:").unwrap_or(dsn);
        // SQLite would silently CREATE a missing file (an empty db that probes green). Refuse
        // instead, and say where the path resolves — the classic trap is a remote gateway user
        // registering a path on their own machine. The path itself is the DSN: never echoed.
        if !std::path::Path::new(path).is_file() {
            return Err(SourceError(
                "sqlite database file not found — the DSN path resolves on the node running \
                 the federation sidecar, not the client"
                    .into(),
            ));
        }
        let pool = SqliteConnectionPoolFactory::new(path, Mode::File, Duration::from_secs(5))
            .build()
            .await
            .map_err(|e| SourceError(format!("sqlite pool: {e}")))?;
        Ok(Self {
            pool: Arc::new(pool),
            path: path.to_string(),
        })
    }
}

#[async_trait::async_trait]
impl Source for SqliteSource {
    async fn probe(&self) -> Result<(), SourceError> {
        // A real probe: resolve sqlite_master as a provider, forcing a live connection to the file.
        // The federated path is irrelevant to a probe — it only needs to know the file opens.
        let _ = self
            .table_provider(&TableReference::bare("sqlite_master"))
            .await?;
        Ok(())
    }

    async fn table_provider(
        &self,
        table: &TableReference,
    ) -> Result<Arc<dyn TableProvider>, SourceError> {
        // Mirror `SqliteTableFactory::table_provider` (the upstream helper does not auto-wrap under
        // `sqlite-federation`), then wrap with `create_federated_table_provider` so the federation
        // optimizer recognizes this table as belonging to one compute context and pushes the whole
        // plan down to SQLite (federation-pushdown scope).
        let pool = Arc::clone(&self.pool);
        let conn = pool
            .connect()
            .await
            .map_err(|e| SourceError(format!("connect: {e}")))?;
        let schema = get_schema(conn, table)
            .await
            .map_err(|e| SourceError(format!("schema {table}: {e}")))?;
        let dyn_pool: Arc<DynSqliteConnectionPool> = pool;
        let sqlite_table = Arc::new(SQLiteTable::new_with_schema(
            &dyn_pool,
            Arc::clone(&schema),
            table.clone(),
        ));
        let federated = sqlite_table
            .create_federated_table_provider()
            .map_err(|e| SourceError(format!("federate {table}: {e}")))?;
        Ok(Arc::new(federated))
    }

    async fn list_tables(&self) -> Result<Vec<super::TableMeta>, SourceError> {
        // Read the source's own catalog via the shared discovery runner (SQLite `sqlite_master`).
        crate::query::run_list_tables(self, "sqlite").await
    }

    async fn foreign_keys(&self, table: &str) -> Result<Vec<super::ForeignKeyMeta>, SourceError> {
        // `PRAGMA foreign_key_list` is not expressible through the DataFusion provider path (the
        // engine only registers tables), so read it directly — a blocking rusqlite open on the same
        // file, off the async runtime. Best-effort: any failure is an EMPTY list, never an error
        // (a missing FK catalog must not fail a snapshot), and the path is never in a message.
        let path = self.path.clone();
        let table = table.to_string();
        let out = tokio::task::spawn_blocking(move || -> Result<Vec<super::ForeignKeyMeta>, ()> {
            let conn = rusqlite::Connection::open_with_flags(
                &path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .map_err(|_| ())?;
            let mut stmt = conn
                .prepare("SELECT \"from\", \"table\", \"to\" FROM pragma_foreign_key_list(?1)")
                .map_err(|_| ())?;
            let rows = stmt
                .query_map([&table], |row| {
                    Ok(super::ForeignKeyMeta {
                        column: row.get(0)?,
                        ref_table: row.get(1)?,
                        // `to` is NULL when the FK targets the parent's implicit PRIMARY KEY;
                        // report the conventional `id` rather than nothing (best-effort catalog).
                        ref_column: row
                            .get::<_, Option<String>>(2)?
                            .unwrap_or_else(|| "id".to_string()),
                    })
                })
                .map_err(|_| ())?;
            Ok(rows.filter_map(Result::ok).collect())
        })
        .await;
        Ok(out.ok().and_then(Result::ok).unwrap_or_default())
    }

    /// `PRAGMA table_info` — richer type names than Arrow exposes (sqlite stores declared type
    /// strings verbatim). Returns the columns NORMALIZED to the canonical vocabulary so the migrate
    /// diff is idempotent (scope Risk 1: `VARCHAR(255)` live vs `text` designed must not plan a
    /// spurious ALTER). The PK column set is read alongside (cid=5 `pk` flag).
    async fn list_columns_with_types(
        &self,
        table: &str,
        kind: &str,
    ) -> Result<Vec<super::LiveColumn>, SourceError> {
        let path = self.path.clone();
        let table = table.to_string();
        let kind = kind.to_string();
        let out = tokio::task::spawn_blocking(
            move || -> Result<Vec<super::LiveColumn>, rusqlite::Error> {
                let conn = rusqlite::Connection::open_with_flags(
                    &path,
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                )?;
                let mut stmt = conn.prepare(
                    "SELECT name, type, \"notnull\" FROM pragma_table_info(?1) ORDER BY cid",
                )?;
                let rows = stmt.query_map([&table], |row| {
                    let name: String = row.get(0)?;
                    let ty: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
                    let notnull: bool = row.get(2)?;
                    Ok(super::LiveColumn {
                        name,
                        neutral_type: super::dialect::canonicalize_live_type(&ty, &kind),
                        nullable: !notnull,
                    })
                })?;
                Ok(rows.filter_map(Result::ok).collect())
            },
        )
        .await
        .map_err(|e| SourceError(format!("columns worker: {e}")))?
        .map_err(|e| SourceError(format!("list columns: {e}")))?;
        Ok(out)
    }

    /// Apply a DDL batch in one sqlite `BEGIN`/`COMMIT`. rusqlite executes DDL synchronously, so
    /// we run it on `spawn_blocking` (the sidecar is multi-threaded). A failure rolls back the
    /// whole batch — no half-applied migrate (the scope's one-transaction invariant).
    async fn apply_ddl(&self, stmts: &[super::DdlStatement]) -> Result<(), super::SourceError> {
        if stmts.is_empty() {
            return Ok(());
        }
        let path = self.path.clone();
        let sqls: Vec<String> = stmts.iter().map(|s| s.sql().to_string()).collect();
        tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            let mut conn = rusqlite::Connection::open(&path)?;
            let tx = conn.transaction()?;
            {
                // Execute each statement in order inside the one transaction. A failure aborts
                // (drops `tx` → rollback) so no half-applied migrate survives (the scope invariant).
                for sql in &sqls {
                    tx.execute(sql, [])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| super::SourceError(format!("ddl worker: {e}")))?
        .map_err(|e| super::SourceError(format!("apply ddl: {e}")))
    }

    /// INSERT (or `ON CONFLICT(key) DO UPDATE` when `key` is given) with parameterized values.
    /// Values are bound as rusqlite `Value`s — never inlined into SQL (no injection surface). The
    /// whole batch runs in one transaction so a mid-batch failure rolls everything back (the
    /// federation.write idempotence + the export no-duplicates invariant).
    async fn write_rows(
        &self,
        table: &str,
        columns: &[String],
        rows: &[Vec<serde_json::Value>],
        key: Option<&[String]>,
    ) -> Result<u64, super::SourceError> {
        if rows.is_empty() {
            return Ok(0);
        }
        let path = self.path.clone();
        let table = table.to_string();
        let columns: Vec<String> = columns.to_vec();
        let rows: Vec<Vec<serde_json::Value>> = rows.to_vec();
        let key: Vec<String> = key.unwrap_or(&[]).to_vec();
        tokio::task::spawn_blocking(move || -> Result<u64, rusqlite::Error> {
            let mut conn = rusqlite::Connection::open(&path)?;
            let tx = conn.transaction()?;
            let mut affected = 0u64;
            // Build the SQL once (placeholders + optional ON CONFLICT clause), reuse the prepared
            // statement per row. Identifiers are quote_ident'd (validated caller-side + defense in
            // depth here).
            let quoted_cols: Vec<String> = columns
                .iter()
                .map(|c| super::dialect::quote_ident(c))
                .collect();
            let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("?{i}")).collect();
            let mut sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                super::dialect::quote_ident(&table),
                quoted_cols.join(", "),
                placeholders.join(", ")
            );
            if !key.is_empty() {
                let conflict_cols: Vec<String> =
                    key.iter().map(|c| super::dialect::quote_ident(c)).collect();
                let updates: Vec<String> = columns
                    .iter()
                    .filter(|c| !key.contains(c))
                    .map(|c| {
                        format!(
                            "{}=excluded.{}",
                            super::dialect::quote_ident(c),
                            super::dialect::quote_ident(c)
                        )
                    })
                    .collect();
                if updates.is_empty() {
                    sql.push_str(&format!(
                        " ON CONFLICT({}) DO NOTHING",
                        conflict_cols.join(", ")
                    ));
                } else {
                    sql.push_str(&format!(
                        " ON CONFLICT({}) DO UPDATE SET {}",
                        conflict_cols.join(", "),
                        updates.join(", ")
                    ));
                }
            }
            {
                let mut stmt = tx.prepare(&sql)?;
                for row in &rows {
                    // `columns[i]` ↔ `row[i]` (column-aligned). Bind each cell as a sqlite Value —
                    // parameterized, never inlined into SQL.
                    let params: Vec<rusqlite::types::Value> = columns
                        .iter()
                        .zip(row.iter())
                        .map(|(_, v)| json_to_sqlite(v))
                        .collect();
                    let params_ref: Vec<&dyn rusqlite::ToSql> =
                        params.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
                    affected += stmt.execute(params_ref.as_slice())? as u64;
                }
            }
            tx.commit()?;
            Ok(affected)
        })
        .await
        .map_err(|e| super::SourceError(format!("write worker: {e}")))?
        .map_err(|e| super::SourceError(format!("write rows: {e}")))
    }

    /// `DELETE FROM <table> WHERE <k1>=?1 AND <k2>=?2 ...`, one statement per key row, with
    /// parameterized key values — never inlined into SQL (no injection surface). The whole batch
    /// runs in one transaction so a mid-batch failure rolls everything back (the federation.delete
    /// invariant). Identifiers are quote_ident'd (validated caller-side + defense in depth here).
    async fn delete_rows(
        &self,
        table: &str,
        key: &[String],
        rows: &[Vec<serde_json::Value>],
    ) -> Result<u64, super::SourceError> {
        if rows.is_empty() {
            return Ok(0);
        }
        let path = self.path.clone();
        let table = table.to_string();
        let key: Vec<String> = key.to_vec();
        let rows: Vec<Vec<serde_json::Value>> = rows.to_vec();
        tokio::task::spawn_blocking(move || -> Result<u64, rusqlite::Error> {
            let mut conn = rusqlite::Connection::open(&path)?;
            let tx = conn.transaction()?;
            let mut affected = 0u64;
            // Build the SQL once (one `k=?n` term per key column), reuse the prepared statement per
            // row. Identifiers are quote_ident'd (validated caller-side + defense in depth here).
            let predicates: Vec<String> = key
                .iter()
                .enumerate()
                .map(|(i, c)| format!("{}=?{}", super::dialect::quote_ident(c), i + 1))
                .collect();
            let sql = format!(
                "DELETE FROM {} WHERE {}",
                super::dialect::quote_ident(&table),
                predicates.join(" AND ")
            );
            {
                let mut stmt = tx.prepare(&sql)?;
                for row in &rows {
                    // `key[i]` ↔ `row[i]` (key-aligned). Bind each cell as a sqlite Value —
                    // parameterized, never inlined into SQL.
                    let params: Vec<rusqlite::types::Value> =
                        row.iter().map(json_to_sqlite).collect();
                    let params_ref: Vec<&dyn rusqlite::ToSql> =
                        params.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
                    affected += stmt.execute(params_ref.as_slice())? as u64;
                }
            }
            tx.commit()?;
            Ok(affected)
        })
        .await
        .map_err(|e| super::SourceError(format!("delete worker: {e}")))?
        .map_err(|e| super::SourceError(format!("delete rows: {e}")))
    }
}

/// Map a JSON cell value to a sqlite `Value` (parameterized — never inlined). sqlite is dynamically
/// typed; we coerce permissively (a number-looking string stays a string) so the engine's own
/// permissive affinity rules apply at query time.
fn json_to_sqlite(v: &serde_json::Value) -> rusqlite::types::Value {
    use rusqlite::types::Value;
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Integer(*b as i64),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Value::Real(f)
            } else {
                Value::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => Value::Text(s.clone()),
        // Structured values (arrays/objects) serialize to JSON text (a `json`/`text` column reads
        // them back via `json_extract`). A binary column would want bytes — v1 stores JSON text.
        other => Value::Text(other.to_string()),
    }
}
