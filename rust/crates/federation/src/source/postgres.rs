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

/// A connected Postgres source: the pool + a table-provider factory over it. The pool is retained
/// (not just the factory) so `apply_ddl`/`write_rows` can open a `connect_direct` connection and
/// run real `execute`/transaction — the read-only DataFusion provider path cannot express a write.
pub struct PostgresSource {
    factory: PostgresTableFactory,
    pool: Arc<PostgresConnectionPool>,
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
        let pool = Arc::new(
            PostgresConnectionPool::new(params)
                .await
                .map_err(|e| SourceError(format!("postgres pool: {e}")))?,
        );
        Ok(Self {
            factory: PostgresTableFactory::new(Arc::clone(&pool)),
            pool,
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

    /// Apply a DDL batch in ONE Postgres transaction (Postgres DDL is transactional — a mid-batch
    /// failure rolls the whole migrate back, the scope's one-transaction invariant). The statements
    /// are the planner's allow-listed output (CREATE TABLE / ADD COLUMN / ADD CONSTRAINT), never
    /// caller SQL, so they run as literal `execute` with no parameters.
    async fn apply_ddl(&self, stmts: &[super::DdlStatement]) -> Result<(), SourceError> {
        if stmts.is_empty() {
            return Ok(());
        }
        let mut conn = self
            .pool
            .connect_direct()
            .await
            .map_err(|e| SourceError(format!("ddl connect: {e}")))?;
        let tx = conn
            .conn
            .transaction()
            .await
            .map_err(|e| SourceError(format!("ddl begin: {e}")))?;
        for stmt in stmts {
            tx.execute(stmt.sql(), &[])
                .await
                .map_err(|e| SourceError(format!("apply ddl: {e}")))?;
        }
        tx.commit()
            .await
            .map_err(|e| SourceError(format!("ddl commit: {e}")))?;
        Ok(())
    }

    /// INSERT (or `ON CONFLICT(key) DO UPDATE` when `key` is given) with parameterized values — the
    /// whole batch in one transaction so a mid-batch failure rolls everything back (federation.write
    /// idempotence + the export no-duplicates invariant). Values are bound as `$n` params, NEVER
    /// inlined into SQL — a caller cannot inject SQL through a cell value.
    async fn write_rows(
        &self,
        table: &str,
        columns: &[String],
        rows: &[Vec<serde_json::Value>],
        key: Option<&[String]>,
    ) -> Result<u64, SourceError> {
        if rows.is_empty() {
            return Ok(0);
        }
        // Build the SQL once (identifiers quote_ident'd — validated caller-side + defense in depth),
        // reuse the prepared statement per row.
        let quoted_cols: Vec<String> = columns
            .iter()
            .map(|c| super::dialect::quote_ident(c))
            .collect();
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("${i}")).collect();
        let mut sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            super::dialect::quote_ident(table),
            quoted_cols.join(", "),
            placeholders.join(", ")
        );
        if let Some(key) = key.filter(|k| !k.is_empty()) {
            let conflict_cols: Vec<String> =
                key.iter().map(|c| super::dialect::quote_ident(c)).collect();
            let updates: Vec<String> = columns
                .iter()
                .filter(|c| !key.contains(c))
                .map(|c| {
                    let q = super::dialect::quote_ident(c);
                    format!("{q}=excluded.{q}")
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

        let mut conn = self
            .pool
            .connect_direct()
            .await
            .map_err(|e| SourceError(format!("write connect: {e}")))?;
        let tx = conn
            .conn
            .transaction()
            .await
            .map_err(|e| SourceError(format!("write begin: {e}")))?;
        let stmt = tx
            .prepare(&sql)
            .await
            .map_err(|e| SourceError(format!("write prepare: {e}")))?;
        let mut affected = 0u64;
        for row in rows {
            // `columns[i]` ↔ `row[i]` (column-aligned). Each cell becomes an owned PgValue that
            // implements ToSql — parameterized, never inlined.
            let params: Vec<PgValue> = row.iter().map(PgValue::from_json).collect();
            let refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = params
                .iter()
                .map(|p| p as &(dyn tokio_postgres::types::ToSql + Sync))
                .collect();
            affected += tx
                .execute(&stmt, refs.as_slice())
                .await
                .map_err(|e| SourceError(format!("write rows: {e}")))?;
        }
        tx.commit()
            .await
            .map_err(|e| SourceError(format!("write commit: {e}")))?;
        Ok(affected)
    }
}

/// An owned cell value bound as a Postgres `$n` parameter (parameterized — never inlined into SQL).
/// Postgres is strictly typed, so we bind each JSON scalar as its natural Postgres type and let the
/// server coerce into the target column (an `i64` into `integer`, `text` into `text`, `f64` into
/// `numeric`/`double`). Structured JSON (arrays/objects) is sent as a `jsonb` value so a `json`/
/// `jsonb` column reads it back structurally.
#[derive(Debug)]
enum PgValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Json(serde_json::Value),
}

impl PgValue {
    fn from_json(v: &serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => PgValue::Null,
            serde_json::Value::Bool(b) => PgValue::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    PgValue::Int(i)
                } else if let Some(f) = n.as_f64() {
                    PgValue::Float(f)
                } else {
                    PgValue::Text(n.to_string())
                }
            }
            serde_json::Value::String(s) => PgValue::Text(s.clone()),
            other => PgValue::Json(other.clone()),
        }
    }
}

impl tokio_postgres::types::ToSql for PgValue {
    fn to_sql(
        &self,
        ty: &tokio_postgres::types::Type,
        out: &mut tokio_postgres::types::private::BytesMut,
    ) -> Result<tokio_postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        match self {
            PgValue::Null => Ok(tokio_postgres::types::IsNull::Yes),
            PgValue::Bool(b) => b.to_sql(ty, out),
            PgValue::Int(i) => i.to_sql(ty, out),
            PgValue::Float(f) => f.to_sql(ty, out),
            PgValue::Text(s) => s.to_sql(ty, out),
            PgValue::Json(j) => j.to_sql(ty, out),
        }
    }

    fn accepts(_ty: &tokio_postgres::types::Type) -> bool {
        // Accept whatever the target column declares — the server drives coercion. Per-arm encoders
        // above still refuse a genuine mismatch at `to_sql` time.
        true
    }

    tokio_postgres::types::to_sql_checked!();
}
