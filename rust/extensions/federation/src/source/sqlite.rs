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

/// A connected SQLite source: a file-backed pool + a table-provider factory over it.
pub struct SqliteSource {
    factory: SqliteTableFactory,
}

impl SqliteSource {
    /// Open a file-backed pool. `dsn` is the database file path (e.g. `/tmp/fed-test.db` or a
    /// `file:/...` URL — the leading `file:` scheme is stripped).
    pub async fn connect(dsn: &str) -> Result<Self, SourceError> {
        let path = dsn.strip_prefix("file:").unwrap_or(dsn);
        let pool = SqliteConnectionPoolFactory::new(path, Mode::File, Duration::from_secs(5))
            .build()
            .await
            .map_err(|e| SourceError(format!("sqlite pool: {e}")))?;
        Ok(Self {
            factory: SqliteTableFactory::new(Arc::new(pool)),
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
}
