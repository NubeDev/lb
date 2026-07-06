//! The SQLite source — a REAL on-disk SQLite engine behind the [`Source`](super::Source) trait
//! (datasources scope testing fallback). When Docker is unavailable to spawn Postgres, the tests use
//! this: a real external database engine with real rows on disk, still behind the one trait — never
//! an in-process re-implementation (testing-scope §0). The "DSN" is the file path.

use std::sync::Arc;
use std::time::Duration;

use datafusion::catalog::TableProvider;
use datafusion::sql::TableReference;
use datafusion_table_providers::sql::db_connection_pool::sqlitepool::SqliteConnectionPoolFactory;
use datafusion_table_providers::sql::db_connection_pool::Mode;
use datafusion_table_providers::sqlite::SqliteTableFactory;

use super::{Source, SourceError};

/// A connected SQLite source: a file-backed pool + a table-provider factory over it. The file path
/// is retained ONLY for direct catalog reads (`PRAGMA foreign_key_list`) — it is the DSN, so it is
/// never echoed into an error or a result.
pub struct SqliteSource {
    factory: SqliteTableFactory,
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
            factory: SqliteTableFactory::new(Arc::new(pool)),
            path: path.to_string(),
        })
    }
}

#[async_trait::async_trait]
impl Source for SqliteSource {
    async fn probe(&self) -> Result<(), SourceError> {
        // A real probe: resolve sqlite_master as a provider, forcing a live connection to the file.
        self.factory
            .table_provider(TableReference::bare("sqlite_master"))
            .await
            .map(|_| ())
            .map_err(|e| SourceError(format!("probe: {e}")))
    }

    async fn table_provider(
        &self,
        table: &TableReference,
    ) -> Result<Arc<dyn TableProvider>, SourceError> {
        self.factory
            .table_provider(table.clone())
            .await
            .map_err(|e| SourceError(format!("table {table}: {e}")))
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
}
