//! The Postgres/Timescale source — the headline datasource (datasources scope). Owns a real
//! `PostgresConnectionPool` behind the [`Source`](super::Source) trait; the DSN is handed in once
//! (host secret-mediation) and lives only inside the pool — never retained where a log/result could
//! observe it. Each referenced table becomes a DataFusion `TableProvider` that pushes the query down
//! to Postgres (adapted from rubix-cube's per-table registration, MIT/Apache-2.0).

use std::collections::HashMap;
use std::sync::Arc;

use datafusion::catalog::TableProvider;
use datafusion::sql::TableReference;
use datafusion_table_providers::postgres::PostgresTableFactory;
use datafusion_table_providers::sql::db_connection_pool::postgrespool::PostgresConnectionPool;
use secrecy::SecretString;

use super::{Source, SourceError};

/// A connected Postgres source: the pool + a table-provider factory over it.
pub struct PostgresSource {
    factory: PostgresTableFactory,
}

impl PostgresSource {
    /// Open the pool from a libpq-style DSN (`postgresql://user:pass@host:port/db?sslmode=…`). The
    /// DSN is moved into the pool params (a `SecretString`) and dropped here — it is not stored on
    /// the struct, so no field can leak it.
    pub async fn connect(dsn: &str) -> Result<Self, SourceError> {
        let mut params: HashMap<String, SecretString> = HashMap::new();
        params.insert(
            "connection_string".into(),
            SecretString::from(dsn.to_string()),
        );
        // A test/dev DSN against a plaintext local container has no TLS; default verify-full would
        // refuse it. The DSN's own `sslmode` (parsed by the pool) governs — we set a permissive
        // default only when the DSN omits it, so a real `sslmode=verify-full` DSN is honored.
        if !dsn.contains("sslmode") {
            params.insert("sslmode".into(), SecretString::from("disable".to_string()));
        }
        let pool = PostgresConnectionPool::new(params)
            .await
            .map_err(|e| SourceError(format!("postgres pool: {e}")))?;
        Ok(Self {
            factory: PostgresTableFactory::new(Arc::new(pool)),
        })
    }
}

#[async_trait::async_trait]
impl Source for PostgresSource {
    async fn probe(&self) -> Result<(), SourceError> {
        // A real connectivity probe: resolve the system catalog as a table provider (forces a live
        // connection). If the pool can build a provider, the endpoint is reachable + authenticated.
        self.factory
            // `parse_str`, not `bare`: a dotted catalog name must split into schema=`pg_catalog`,
            // table=`pg_tables`. `bare` would keep the literal `pg_catalog.pg_tables` as a single
            // table name, which the provider can't introspect (it reports an empty schema).
            .table_provider(TableReference::parse_str("pg_catalog.pg_tables"))
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
        // Read the source's own catalog via the shared discovery runner (Postgres `pg_catalog`).
        crate::query::run_list_tables(self, "postgres").await
    }

    async fn foreign_keys(&self, table: &str) -> Result<Vec<super::ForeignKeyMeta>, SourceError> {
        // FK metadata via the standard `information_schema` views, joined in the engine through the
        // same catalog runner discovery uses. Best-effort: any failure (a view the pushdown provider
        // can't introspect, an odd remote) is an EMPTY list, never an error — a missing FK catalog
        // must not fail a `federation.sample` snapshot.
        let t = table.replace('\'', "''");
        let sql = format!(
            "SELECT kcu.column_name AS col, ccu.table_name AS ref_table, \
                    ccu.column_name AS ref_col \
             FROM __tc__ tc \
             JOIN __kcu__ kcu ON tc.constraint_name = kcu.constraint_name \
             JOIN __ccu__ ccu ON tc.constraint_name = ccu.constraint_name \
             WHERE tc.constraint_type = 'FOREIGN KEY' AND tc.table_name = '{t}' \
             ORDER BY kcu.column_name"
        );
        let bindings = [
            ("__tc__", "information_schema.table_constraints"),
            ("__kcu__", "information_schema.key_column_usage"),
            ("__ccu__", "information_schema.constraint_column_usage"),
        ];
        let rows = crate::query::catalog_rows(self, &sql, &bindings)
            .await
            .unwrap_or_default();
        Ok(rows
            .into_iter()
            .filter_map(|obj| {
                Some(super::ForeignKeyMeta {
                    column: obj.get("col")?.as_str()?.to_string(),
                    ref_table: obj.get("ref_table")?.as_str()?.to_string(),
                    ref_column: obj.get("ref_col")?.as_str()?.to_string(),
                })
            })
            .collect())
    }
}
