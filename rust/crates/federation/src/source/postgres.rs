//! The Postgres/Timescale source — the headline datasource (datasources scope). Owns a real
//! `PostgresConnectionPool` behind the [`Source`](super::Source) trait; the DSN is handed in once
//! (host secret-mediation) and lives only inside the pool — never retained where a log/result could
//! observe it. Each referenced table becomes a DataFusion `TableProvider` that pushes the query down
//! to Postgres (adapted from rubix-cube's per-table registration, MIT/Apache-2.0).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use datafusion::catalog::TableProvider;
use datafusion::sql::TableReference;
use datafusion_table_providers::postgres::PostgresTableFactory;
use datafusion_table_providers::sql::db_connection_pool::postgrespool::PostgresConnectionPool;
use secrecy::SecretString;

use super::{Source, SourceError};

use arrow::array::*;
use arrow::datatypes::{DataType, TimeUnit};
use arrow::record_batch::RecordBatch;
use datafusion_table_providers::sql::db_connection_pool::{
    dbconnection::query_arrow, DbConnectionPool,
};
use futures::TryStreamExt;

/// A connected Postgres source: the pool + a table-provider factory over it. The pool is retained
/// (not just the factory) so `apply_ddl`/`write_rows` can open a `connect_direct` connection and
/// run real `execute`/transaction — the read-only DataFusion provider path cannot express a write.
pub struct PostgresSource {
    factory: PostgresTableFactory,
    pool: Arc<PostgresConnectionPool>,
    provider_cache: Mutex<HashMap<String, Arc<dyn TableProvider>>>,
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
            provider_cache: Mutex::new(HashMap::new()),
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
        let table_name = table.to_string();
        if let Some(cached) = self
            .provider_cache
            .lock()
            .expect("cache mutex")
            .get(&table_name)
        {
            return Ok(cached.clone());
        }
        let provider = self
            .factory
            .table_provider(table.clone())
            .await
            .map_err(|e| SourceError(format!("table {table}: {e}")))?;
        self.provider_cache
            .lock()
            .expect("cache mutex")
            .insert(table_name, provider.clone());
        Ok(provider)
    }

    async fn exec_raw_for_test(&self, sql: &str) -> Result<(), SourceError> {
        // Real write connection (same `connect_direct` path apply_ddl/write_rows use) so a test seeds
        // through the actual pool, not a shortcut. Internal test SQL only — never a caller surface.
        let conn = self
            .pool
            .connect_direct()
            .await
            .map_err(|e| SourceError(format!("exec connect: {e}")))?;
        conn.conn
            .batch_execute(sql)
            .await
            .map_err(|e| SourceError(format!("exec: {e}")))
    }

    async fn explain_for_test(&self, sql: &str) -> Result<String, SourceError> {
        let conn = self
            .pool
            .connect_direct()
            .await
            .map_err(|e| SourceError(format!("explain connect: {e}")))?;
        let rows = conn
            .conn
            .query(&format!("EXPLAIN {sql}"), &[])
            .await
            .map_err(|e| SourceError(format!("explain: {e}")))?;
        let mut plan = String::new();
        for row in &rows {
            let line: &str = row.get(0);
            plan.push_str(line);
            plan.push('\n');
        }
        Ok(plan)
    }

    async fn query_direct(&self, sql: &str) -> Result<Vec<RecordBatch>, SourceError> {
        let conn = self
            .pool
            .connect()
            .await
            .map_err(|e| SourceError(format!("direct connect: {e}")))?;
        let stream = query_arrow(conn, sql.to_string(), None)
            .await
            .map_err(|e| SourceError(format!("direct query: {e}")))?;
        stream
            .try_collect()
            .await
            .map_err(|e| SourceError(format!("direct collect: {e}")))
    }

    async fn query_direct_json(
        &self,
        sql: &str,
    ) -> Result<(Vec<String>, Vec<serde_json::Value>), SourceError> {
        let batches = self.query_direct(sql).await?;
        Ok(batches_to_column_rows(&batches))
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

    /// `DELETE FROM <table> WHERE <k1>=$1 AND <k2>=$2 ...`, one statement per key row, with
    /// parameterized `$n` key values — never inlined into SQL. The whole batch runs in one
    /// transaction so a mid-batch failure rolls everything back (the federation.delete invariant).
    /// Identifiers are quote_ident'd (validated caller-side + defense in depth here).
    async fn delete_rows(
        &self,
        table: &str,
        key: &[String],
        rows: &[Vec<serde_json::Value>],
    ) -> Result<u64, SourceError> {
        if rows.is_empty() {
            return Ok(0);
        }
        // Build the SQL once (one `k=$n` term per key column), reuse the prepared statement per row.
        let predicates: Vec<String> = key
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{}=${}", super::dialect::quote_ident(c), i + 1))
            .collect();
        let sql = format!(
            "DELETE FROM {} WHERE {}",
            super::dialect::quote_ident(table),
            predicates.join(" AND ")
        );

        let mut conn = self
            .pool
            .connect_direct()
            .await
            .map_err(|e| SourceError(format!("delete connect: {e}")))?;
        let tx = conn
            .conn
            .transaction()
            .await
            .map_err(|e| SourceError(format!("delete begin: {e}")))?;
        let stmt = tx
            .prepare(&sql)
            .await
            .map_err(|e| SourceError(format!("delete prepare: {e}")))?;
        let mut affected = 0u64;
        for row in rows {
            // `key[i]` ↔ `row[i]` (key-aligned). Each cell becomes an owned PgValue that implements
            // ToSql — parameterized, never inlined.
            let params: Vec<PgValue> = row.iter().map(PgValue::from_json).collect();
            let refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = params
                .iter()
                .map(|p| p as &(dyn tokio_postgres::types::ToSql + Sync))
                .collect();
            affected += tx
                .execute(&stmt, refs.as_slice())
                .await
                .map_err(|e| SourceError(format!("delete rows: {e}")))?;
        }
        tx.commit()
            .await
            .map_err(|e| SourceError(format!("delete commit: {e}")))?;
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

/// Convert Arrow RecordBatches to column-aligned `(columns, rows)` by iterating each
/// column's typed Arrow array — no JSON text intermediate, no per-cell Postgres OID dispatch.
fn batches_to_column_rows(batches: &[RecordBatch]) -> (Vec<String>, Vec<serde_json::Value>) {
    let columns: Vec<String> = match batches.first() {
        Some(b) => b
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect(),
        None => return (Vec::new(), Vec::new()),
    };

    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    let mut rows = Vec::with_capacity(total_rows);

    for batch in batches {
        for r in 0..batch.num_rows() {
            let mut vals = Vec::with_capacity(columns.len());
            for c in 0..batch.num_columns() {
                vals.push(cell_to_value(batch.column(c).as_ref(), r));
            }
            rows.push(serde_json::Value::Array(vals));
        }
    }

    (columns, rows)
}

/// Convert one Arrow cell to a JSON Value by dispatching on the column's Arrow DataType.
/// NULL cells are handled before dispatch; typed array access replaces the prior row-by-row
/// string-based Postgres OID match + per-cell `try_get` decode.
fn cell_to_value(col: &dyn arrow::array::Array, row: usize) -> serde_json::Value {
    if col.is_null(row) {
        return serde_json::Value::Null;
    }
    match col.data_type() {
        DataType::Boolean => {
            let a = col.as_any().downcast_ref::<BooleanArray>().unwrap();
            serde_json::Value::Bool(a.value(row))
        }
        DataType::Int16 => {
            let a = col.as_any().downcast_ref::<Int16Array>().unwrap();
            serde_json::json!(a.value(row))
        }
        DataType::Int32 => {
            let a = col.as_any().downcast_ref::<Int32Array>().unwrap();
            serde_json::json!(a.value(row))
        }
        DataType::Int64 => {
            let a = col.as_any().downcast_ref::<Int64Array>().unwrap();
            serde_json::json!(a.value(row))
        }
        DataType::Float32 => {
            let a = col.as_any().downcast_ref::<Float32Array>().unwrap();
            serde_json::json!(a.value(row))
        }
        DataType::Float64 => {
            let a = col.as_any().downcast_ref::<Float64Array>().unwrap();
            serde_json::json!(a.value(row))
        }
        DataType::Utf8 => {
            let a = col.as_any().downcast_ref::<StringArray>().unwrap();
            serde_json::Value::String(a.value(row).to_string())
        }
        DataType::LargeUtf8 => {
            let a = col.as_any().downcast_ref::<LargeStringArray>().unwrap();
            serde_json::Value::String(a.value(row).to_string())
        }
        DataType::Timestamp(unit, tz_override) => {
            let (secs, nsecs) = match unit {
                TimeUnit::Second => {
                    let a = col.as_any().downcast_ref::<TimestampSecondArray>().unwrap();
                    (a.value(row), 0)
                }
                TimeUnit::Millisecond => {
                    let a = col
                        .as_any()
                        .downcast_ref::<TimestampMillisecondArray>()
                        .unwrap();
                    let ms = a.value(row);
                    (ms / 1000, ((ms % 1000) * 1_000_000) as u32)
                }
                TimeUnit::Microsecond => {
                    let a = col
                        .as_any()
                        .downcast_ref::<TimestampMicrosecondArray>()
                        .unwrap();
                    let us = a.value(row);
                    (us / 1_000_000, ((us % 1_000_000) * 1_000) as u32)
                }
                TimeUnit::Nanosecond => {
                    let a = col
                        .as_any()
                        .downcast_ref::<TimestampNanosecondArray>()
                        .unwrap();
                    let ns = a.value(row);
                    (ns / 1_000_000_000, (ns % 1_000_000_000) as u32)
                }
            };
            match chrono::DateTime::from_timestamp(secs, nsecs) {
                Some(dt) => {
                    if tz_override.is_some() {
                        // Match the DataFusion path's wire form exactly: `arrow_json` renders a
                        // tz-aware timestamp as RFC3339 with a `Z` suffix for UTC (`…05Z`), not
                        // `+00:00`. `to_rfc3339()` would emit `+00:00` and diverge from every value a
                        // dashboard already got via the DataFusion path — a spurious "changed" for the
                        // same instant. `to_rfc3339_opts(_, use_z=true)` gives the `Z` form.
                        serde_json::Value::String(
                            dt.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
                        )
                    } else {
                        serde_json::Value::String(dt.naive_utc().to_string())
                    }
                }
                None => serde_json::Value::Null,
            }
        }
        DataType::Date32 => {
            let a = col.as_any().downcast_ref::<Date32Array>().unwrap();
            let days = a.value(row);
            // Arrow Date32 = days since epoch; chrono CE days = epoch + 719163
            let ce_days = days + 719_163;
            let date = chrono::NaiveDate::from_num_days_from_ce_opt(ce_days)
                .map(|d| d.to_string())
                .unwrap_or_default();
            serde_json::Value::String(date)
        }
        DataType::Date64 => {
            let a = col.as_any().downcast_ref::<Date64Array>().unwrap();
            let ms = a.value(row);
            let ce_days = (ms / 86_400_000) as i32 + 719_163;
            let date = chrono::NaiveDate::from_num_days_from_ce_opt(ce_days)
                .map(|d| d.to_string())
                .unwrap_or_default();
            serde_json::Value::String(date)
        }
        DataType::Time32(unit) => {
            let a = col
                .as_any()
                .downcast_ref::<Time32MillisecondArray>()
                .unwrap();
            let val = a.value(row);
            let secs = match unit {
                TimeUnit::Second => val,
                TimeUnit::Millisecond => val / 1000,
                _ => val / 1000,
            };
            let time = chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs as u32, 0)
                .map(|t| t.to_string())
                .unwrap_or_default();
            serde_json::Value::String(time)
        }
        DataType::Time64(unit) => {
            let a = col
                .as_any()
                .downcast_ref::<Time64NanosecondArray>()
                .unwrap();
            let ns = a.value(row);
            let secs = match unit {
                TimeUnit::Microsecond | TimeUnit::Nanosecond => ns / 1_000_000_000,
                _ => ns / 1_000_000_000,
            };
            let remaining_ns = match unit {
                TimeUnit::Microsecond => ((ns % 1_000_000_000) * 1_000) as u32,
                TimeUnit::Nanosecond => (ns % 1_000_000_000) as u32,
                _ => 0,
            };
            let time =
                chrono::NaiveTime::from_num_seconds_from_midnight_opt(secs as u32, remaining_ns)
                    .map(|t| t.to_string())
                    .unwrap_or_default();
            serde_json::Value::String(time)
        }
        DataType::Decimal128(_, _) => {
            let a = col.as_any().downcast_ref::<Decimal128Array>().unwrap();
            let val = a.value(row);
            let scale = a.scale();
            let as_f64 = val as f64 / 10f64.powi(scale as i32);
            serde_json::json!(as_f64)
        }
        // Any Arrow type not given an explicit arm above (jsonb, uuid, arrays, interval, bytea,
        // network types, enums, a `numeric` too wide for Decimal128, …). The prior catch-all
        // returned `Null` here, which SILENTLY DROPPED every such cell — invisible data loss in a
        // dashboard panel, and a divergence from the DataFusion path (which renders these via
        // arrow_json). Instead render a best-effort TEXT form via Arrow's own display formatter,
        // which handles lists/structs/decimals/etc. The cell was already proven non-null at the top
        // of this fn, so this branch never fabricates a value for a genuinely-null cell.
        _ => stringify_cell(col, row),
    }
}

/// Best-effort text rendering of one Arrow cell whose `DataType` has no explicit JSON mapping above.
/// Uses `arrow::util::display::ArrayFormatter` — the same machinery `arrow`'s pretty-printer uses —
/// so a `jsonb`/`uuid`/array/interval value becomes its readable string instead of vanishing to
/// `null`. On the (unexpected) event the formatter itself can't be built, fall back to the type name
/// so the loss is still VISIBLE (a marker string), never a silent `null`.
fn stringify_cell(col: &dyn arrow::array::Array, row: usize) -> serde_json::Value {
    use arrow::util::display::{ArrayFormatter, FormatOptions};
    match ArrayFormatter::try_new(col, &FormatOptions::default()) {
        Ok(fmt) => serde_json::Value::String(fmt.value(row).to_string()),
        Err(_) => serde_json::Value::String(format!("<{}>", col.data_type())),
    }
}
