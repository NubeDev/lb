//! `run_query` — the engine-agnostic orchestration of a federated read (datasources scope). It
//! validates SELECT-only, connects the source (DSN handed in, never stored/logged), registers each
//! referenced table as a DataFusion `TableProvider`, runs the query through a `SessionContext`, and
//! returns `{columns, rows}` bounded by the row cap. The pattern (embed the engine, register
//! per-table providers, run validated SQL) is adapted from rubix-cube (MIT/Apache-2.0).

use arrow::record_batch::RecordBatch;
use datafusion::prelude::SessionContext;
use datafusion::sql::TableReference;
use serde_json::Value;

use crate::source::{connect, Source};
use crate::validate::{validate_select, ROW_CAP};

/// The result of a federated query: the column names and the rows (each an array of JSON cells,
/// column-aligned). Bounded to [`ROW_CAP`] rows.
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
}

/// Run `sql` against the `kind` source at `dsn`. Validates SELECT-only first, registers only the
/// tables the query references, and caps the result. The DSN lives only inside the pool.
pub async fn run_query(kind: &str, dsn: &str, sql: &str) -> Result<QueryResult, String> {
    let tables = validate_select(sql).map_err(|e| e.to_string())?;
    let source = connect(kind, dsn).await.map_err(|e| e.to_string())?;
    register_and_run(source.as_ref(), &tables, sql).await
}

/// A real connectivity probe for the `kind` source at `dsn` — `Ok(())` is green.
pub async fn probe(kind: &str, dsn: &str) -> Result<(), String> {
    let source = connect(kind, dsn).await.map_err(|e| e.to_string())?;
    source.probe().await.map_err(|e| e.to_string())
}

/// Register each referenced table into a fresh `SessionContext`, run the SQL, and shape the result.
async fn register_and_run(
    source: &dyn Source,
    tables: &[String],
    sql: &str,
) -> Result<QueryResult, String> {
    let ctx = SessionContext::new();
    for table in tables {
        let reference = TableReference::bare(table.clone());
        let provider = source
            .table_provider(&reference)
            .await
            .map_err(|e| e.to_string())?;
        ctx.register_table(reference, provider)
            .map_err(|e| format!("register {table}: {e}"))?;
    }

    let df = ctx.sql(sql).await.map_err(|e| format!("plan: {e}"))?;
    // Cap before collect: the engine stops materializing past the cap (no unbounded read in a
    // handler — an unbounded export is a mirror job, §6.1).
    let df = df.limit(0, Some(ROW_CAP)).map_err(|e| e.to_string())?;
    let batches = df.collect().await.map_err(|e| format!("execute: {e}"))?;
    shape(batches)
}

/// Convert collected Arrow batches into `{columns, rows}`. Columns come from the first batch's
/// schema; rows are JSON objects flattened to column-aligned arrays.
fn shape(batches: Vec<RecordBatch>) -> Result<QueryResult, String> {
    let columns: Vec<String> = match batches.first() {
        Some(b) => b
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect(),
        None => Vec::new(),
    };

    // arrow-json writes each row as a JSON object keyed by column name; re-project to a
    // column-aligned array so the wire shape is `{columns:[...], rows:[[...], ...]}`.
    let mut buf = Vec::new();
    {
        let mut writer = arrow_json::ArrayWriter::new(&mut buf);
        for batch in &batches {
            writer.write(batch).map_err(|e| e.to_string())?;
        }
        writer.finish().map_err(|e| e.to_string())?;
    }
    let objs: Vec<Value> = if buf.is_empty() {
        Vec::new()
    } else {
        serde_json::from_slice(&buf).map_err(|e| e.to_string())?
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

    Ok(QueryResult { columns, rows })
}
