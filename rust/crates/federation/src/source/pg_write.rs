//! Running a generated write or delete against postgres (`write_sql` builds the statement): one
//! prepared statement, one execution per row, one transaction. Split out of `postgres.rs`, which owns
//! the pool and the `Source` impl — the sqlite twin is `sqlite::run_rows` (FILE-LAYOUT §9).

use datafusion_table_providers::sql::db_connection_pool::postgrespool::PostgresConnectionPool;

use super::pg_value::PgValue;
use super::{SourceError, WriteGuard};

/// Each row's cells bind as parameters, then the guard's ids. A guarded upsert must change exactly
/// its one row (zero means an out-of-scope row already holds the key), else the transaction is
/// dropped uncommitted and the whole batch rolls back. Returns rows changed.
pub(super) async fn run_rows(
    pool: &PostgresConnectionPool,
    sql: &str,
    rows: &[Vec<serde_json::Value>],
    guard: Option<&WriteGuard>,
    each_row_must_change: bool,
) -> Result<u64, SourceError> {
    let err = |what: &str, e: &dyn std::fmt::Display| SourceError(format!("write {what}: {e}"));
    let mut conn = pool
        .connect_direct()
        .await
        .map_err(|e| err("connect", &e))?;
    let tx = conn
        .conn
        .transaction()
        .await
        .map_err(|e| err("begin", &e))?;
    let stmt = tx.prepare(sql).await.map_err(|e| err("prepare", &e))?;
    let ids: Vec<PgValue> = guard
        .iter()
        .flat_map(|g| g.ids.iter())
        .map(|id| PgValue::from_json(&serde_json::Value::String(id.clone())))
        .collect();
    let mut affected = 0u64;
    for (i, row) in rows.iter().enumerate() {
        let params: Vec<PgValue> = row.iter().map(PgValue::from_json).collect();
        let refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = params
            .iter()
            .chain(ids.iter())
            .map(|p| p as &(dyn tokio_postgres::types::ToSql + Sync))
            .collect();
        let n = tx
            .execute(&stmt, refs.as_slice())
            .await
            .map_err(|e| err("rows", &e))?;
        if guard.is_some() && each_row_must_change && n != 1 {
            return Err(SourceError(format!(
                "write rows: denied: rows[{i}] would change a row outside the caller's entities"
            )));
        }
        affected += n;
    }
    tx.commit().await.map_err(|e| err("commit", &e))?;
    Ok(affected)
}
