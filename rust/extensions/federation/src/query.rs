//! `run_query` — the engine-agnostic orchestration of a federated read (datasources scope). It
//! validates SELECT-only, connects the source (DSN handed in, never stored/logged), registers each
//! referenced table as a DataFusion `TableProvider`, runs the query through a `SessionContext`, and
//! returns `{columns, rows}` bounded by the row cap. The pattern (embed the engine, register
//! per-table providers, run validated SQL) is adapted from rubix-cube (MIT/Apache-2.0).
//!
//! `discover_*` — the `federation.schema` discovery path: reuses the SAME table-provider factory to
//! read each source's own catalog (Postgres `pg_catalog` / SQLite `sqlite_master`) and to read a
//! table's Arrow schema for columns. It does NOT issue `information_schema` SQL (the engine registers
//! only the tables a query references, so a virtual catalog is unreachable); it goes through the real
//! remote engine the provider pushes down to.

use arrow::record_batch::RecordBatch;
use datafusion::prelude::SessionContext;
use datafusion::sql::TableReference;
use serde_json::Value;

use crate::source::{connect, ColumnMeta, Source, SourceError, TableMeta};
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

// ───────────────────────────── discovery (`federation.schema`) ─────────────────────────────

/// Run a discovery SELECT that reads catalog tables, returning JSON OBJECT rows (keyed by column
/// name). Each `(alias, remote)` binding builds a provider for the remote catalog table (the same
/// factory `probe` uses) and registers it under the bare `alias`, so the SQL references the alias —
/// this decouples DataFusion's name resolution from the remote catalog's dotted names.
async fn catalog_rows(
    source: &dyn Source,
    sql: &str,
    bindings: &[(&str, &str)],
) -> Result<Vec<Value>, String> {
    let ctx = SessionContext::new();
    for (alias, remote) in bindings {
        // `parse_str` for the remote: catalog names are dotted (`pg_catalog.pg_tables`) and must
        // split into schema + table so the provider introspects the real catalog (a `bare` dotted
        // name reports an empty schema). The alias is a single bare identifier the SQL references.
        let provider = source
            .table_provider(&TableReference::parse_str(remote))
            .await
            .map_err(|e| e.to_string())?;
        ctx.register_table(TableReference::bare(*alias), provider)
            .map_err(|e| format!("register {alias}: {e}"))?;
    }
    let df = ctx.sql(sql).await.map_err(|e| format!("plan: {e}"))?;
    let df = df.limit(0, Some(ROW_CAP)).map_err(|e| e.to_string())?;
    let batches = df.collect().await.map_err(|e| format!("execute: {e}"))?;
    rows_as_objects(batches)
}

/// Collect Arrow batches into JSON OBJECT rows keyed by column name (the discovery result shape).
fn rows_as_objects(batches: Vec<RecordBatch>) -> Result<Vec<Value>, String> {
    let mut buf = Vec::new();
    {
        let mut writer = arrow_json::ArrayWriter::new(&mut buf);
        for batch in &batches {
            writer.write(batch).map_err(|e| e.to_string())?;
        }
        writer.finish().map_err(|e| e.to_string())?;
    }
    if buf.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_slice(&buf).map_err(|e| e.to_string())
}

/// Discover the user tables in the source via its own catalog. The list SQL + bindings are
/// per-source-kind; the orchestration is shared.
pub async fn discover_tables(kind: &str, dsn: &str) -> Result<Vec<TableMeta>, String> {
    let source = connect(kind, dsn).await.map_err(|e| e.to_string())?;
    source.list_tables().await.map_err(|e| e.to_string())
}

/// Discover one table's columns by reading its `TableProvider` Arrow schema — engine-agnostic (works
/// for Postgres and SQLite alike; the provider pushes down and reports the real remote schema).
pub async fn describe_table(kind: &str, dsn: &str, table: &str) -> Result<Vec<ColumnMeta>, String> {
    let source = connect(kind, dsn).await.map_err(|e| e.to_string())?;
    let provider = source
        .table_provider(&TableReference::bare(table))
        .await
        .map_err(|e| e.to_string())?;
    let schema = provider.schema();
    let cols = schema
        .fields()
        .iter()
        .map(|f| ColumnMeta {
            name: f.name().clone(),
            data_type: f.data_type().to_string(),
            nullable: f.is_nullable(),
        })
        .collect();
    Ok(cols)
}

/// Build `TableMeta` rows from JSON objects (a `{name, rows?}` shape, tolerant of missing rows).
pub fn table_meta_from_rows(rows: Vec<Value>) -> Vec<TableMeta> {
    rows.into_iter()
        .filter_map(|obj| {
            let name = obj.get("name")?.as_str()?.to_string();
            let rows = obj
                .get("rows")
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|n| n as i64)));
            Some(TableMeta { name, rows })
        })
        .collect()
}

// Provide the per-kind list query so each `Source` impl stays small and the catalog SQL lives in one
// place. Returns `(sql, bindings)`.
pub(crate) fn list_tables_plan(
    kind: &str,
) -> Result<(&'static str, Vec<(&'static str, &'static str)>), String> {
    match kind {
        "sqlite" => Ok((
            "SELECT name AS name FROM __sm__ WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            vec![("__sm__", "sqlite_master")],
        )),
        // List names from `pg_tables` only. The earlier `reltuples` estimate joined `pg_class`, but
        // the pushed-down `pg_class` provider doesn't expose `relname` to the DataFusion plan
        // (`No field named c.relname`), which broke the whole listing. Names are what the browse panel
        // needs; a row estimate is a nice-to-have we drop rather than fail the list over.
        "postgres" | "timescale" => Ok((
            "SELECT tablename AS name FROM __pg_tables__ WHERE schemaname = 'public' ORDER BY tablename",
            vec![("__pg_tables__", "pg_catalog.pg_tables")],
        )),
        other => Err(format!("unknown source kind: {other}")),
    }
}

/// Run the per-kind list query via the shared catalog runner. Used by `Source::list_tables` impls so
/// they share one orchestration path.
pub async fn run_list_tables(
    source: &dyn Source,
    kind: &str,
) -> Result<Vec<TableMeta>, SourceError> {
    let (sql, bindings) = list_tables_plan(kind).map_err(SourceError)?;
    let rows = catalog_rows(source, sql, &bindings)
        .await
        .map_err(SourceError)?;
    Ok(table_meta_from_rows(rows))
}
