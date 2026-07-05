//! Synthesized read-only `information_schema.tables` / `information_schema.columns` views
//! (datasources scope). Every OpenAI-schooled model probes `information_schema` before anything
//! else; steering it away just burned agent turns (live: three identical rejected probes in one
//! run). Instead the two views it actually wants are answered FOR REAL: in-memory tables built
//! from the source's own catalog (the same `list_tables`/provider-schema reads `federation.schema`
//! does), registered under an `information_schema` schema in the per-query `SessionContext`. Still
//! strictly read-only — the views are ephemeral copies, never a passthrough to `pg_catalog`.

use std::sync::Arc;

use arrow::array::{Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use datafusion::catalog::{MemorySchemaProvider, SchemaProvider};
use datafusion::datasource::MemTable;
use datafusion::prelude::SessionContext;
use datafusion::sql::TableReference;

use crate::source::Source;

/// Register the requested `information_schema` views into `ctx`, built from `source`'s real
/// catalog. A table whose schema cannot be read is skipped (best-effort catalog, never a failed
/// query over one broken table).
pub async fn register_information_schema(
    ctx: &SessionContext,
    source: &dyn Source,
    want_tables: bool,
    want_columns: bool,
) -> Result<(), String> {
    if !want_tables && !want_columns {
        return Ok(());
    }
    let names: Vec<String> = source
        .list_tables()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|t| t.name)
        .collect();

    let schema_provider = Arc::new(MemorySchemaProvider::new());
    if want_tables {
        register(&schema_provider, "tables", tables_batch(&names)?)?;
    }
    if want_columns {
        register(
            &schema_provider,
            "columns",
            columns_batch(source, &names).await?,
        )?;
    }
    // The default catalog is resolved via the context's options (never hardcode "datafusion").
    let catalog_name = ctx.state().config().options().catalog.default_catalog.clone();
    ctx.catalog(&catalog_name)
        .ok_or_else(|| format!("no catalog {catalog_name}"))?
        .register_schema("information_schema", schema_provider)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Put one batch behind a `MemTable` under `name` in the schema provider.
fn register(
    provider: &Arc<MemorySchemaProvider>,
    name: &str,
    batch: RecordBatch,
) -> Result<(), String> {
    let table = MemTable::try_new(batch.schema(), vec![vec![batch]]).map_err(|e| e.to_string())?;
    provider
        .register_table(name.to_string(), Arc::new(table))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// `information_schema.tables` — the standard columns a model filters on. Every row is a user
/// table in the source (`table_schema = 'public'`, `table_type = 'BASE TABLE'`).
fn tables_batch(names: &[String]) -> Result<RecordBatch, String> {
    let n = names.len();
    let schema = Arc::new(Schema::new(vec![
        Field::new("table_catalog", DataType::Utf8, false),
        Field::new("table_schema", DataType::Utf8, false),
        Field::new("table_name", DataType::Utf8, false),
        Field::new("table_type", DataType::Utf8, false),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec!["def"; n])),
            Arc::new(StringArray::from(vec!["public"; n])),
            Arc::new(StringArray::from(
                names.iter().map(String::as_str).collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(vec!["BASE TABLE"; n])),
        ],
    )
    .map_err(|e| e.to_string())
}

/// `information_schema.columns` — one row per column per user table, read from each table's real
/// provider schema (the same read `federation.schema {source, table}` serves).
async fn columns_batch(source: &dyn Source, names: &[String]) -> Result<RecordBatch, String> {
    let mut table_name = Vec::new();
    let mut column_name = Vec::new();
    let mut ordinal = Vec::new();
    let mut data_type = Vec::new();
    let mut is_nullable = Vec::new();
    for name in names {
        let Ok(provider) = source.table_provider(&TableReference::bare(name.clone())).await else {
            continue; // best-effort: one unreadable table must not fail the whole catalog
        };
        for (i, field) in provider.schema().fields().iter().enumerate() {
            table_name.push(name.clone());
            column_name.push(field.name().clone());
            ordinal.push((i + 1) as i64);
            data_type.push(field.data_type().to_string());
            is_nullable.push(if field.is_nullable() { "YES" } else { "NO" });
        }
    }
    let n = table_name.len();
    let schema = Arc::new(Schema::new(vec![
        Field::new("table_catalog", DataType::Utf8, false),
        Field::new("table_schema", DataType::Utf8, false),
        Field::new("table_name", DataType::Utf8, false),
        Field::new("column_name", DataType::Utf8, false),
        Field::new("ordinal_position", DataType::Int64, false),
        Field::new("data_type", DataType::Utf8, false),
        Field::new("is_nullable", DataType::Utf8, false),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec!["def"; n])),
            Arc::new(StringArray::from(vec!["public"; n])),
            Arc::new(StringArray::from(table_name)),
            Arc::new(StringArray::from(column_name)),
            Arc::new(Int64Array::from(ordinal)),
            Arc::new(StringArray::from(data_type)),
            Arc::new(StringArray::from(is_nullable)),
        ],
    )
    .map_err(|e| e.to_string())
}
