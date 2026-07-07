//! `run_sample` — the `federation.sample` engine pass (datasource-samples scope): ONE bounded
//! snapshot of a source for an AI prompt — every table's columns, its foreign keys (best-effort,
//! per-kind), and up to `limit` real rows. One `connect` serves the whole snapshot (the host makes
//! one sidecar call, not N+1). Hard-bounded and prompt-sized: table/row caps, long cells truncated,
//! and sensitive-looking columns redacted by name — the snapshot is DESTINED for a model prompt, so
//! it travels further than a query result.

use datafusion::prelude::SessionContext;
use datafusion::sql::TableReference;
use serde_json::{json, Value};

use crate::source::{connect, ForeignKeyMeta};

/// At most this many tables per snapshot (deterministic order; `truncated: true` when cut).
pub const MAX_TABLES: usize = 25;
/// At most this many sample rows per table (`limit` is clamped host-side too).
pub const MAX_ROWS: usize = 50;
/// Default sample rows per table when the caller passes no `limit`.
pub const DEFAULT_ROWS: usize = 10;
/// A string cell longer than this is truncated (with an ellipsis) — the size backstop.
const MAX_CELL_CHARS: usize = 256;
/// A column whose lowercased name contains one of these is emitted as `«redacted»` — a fixed
/// built-in denylist (scope: fixed first, a per-datasource record field only when asked for).
const REDACT: &[&str] = &["password", "secret", "token", "api_key", "apikey"];
const REDACTED: &str = "«redacted»";

/// Build the snapshot for the `kind` source at `dsn`: `{source-agnostic tables[], relationships[],
/// truncated}`. `tables` filters to the named tables when present; `limit` is clamped to
/// [`MAX_ROWS`]. Per-table reads are best-effort — one unreadable table is skipped, never a failed
/// snapshot (the `info_schema.rs` stance). The DSN lives only inside the pool.
pub async fn run_sample(
    kind: &str,
    dsn: &str,
    tables: Option<Vec<String>>,
    limit: usize,
) -> Result<Value, String> {
    let limit = limit.clamp(1, MAX_ROWS);
    let source = connect(kind, dsn).await.map_err(|e| e.to_string())?;

    let mut names: Vec<String> = source
        .list_tables()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|t| t.name)
        .collect();
    if let Some(filter) = &tables {
        names.retain(|n| filter.iter().any(|f| f == n));
    }
    let truncated = names.len() > MAX_TABLES;
    names.truncate(MAX_TABLES);

    let mut out_tables = Vec::new();
    let mut relationships = Vec::new();
    for name in &names {
        let Ok(provider) = source
            .table_provider(&TableReference::bare(name.clone()))
            .await
        else {
            continue; // best-effort: one unreadable table must not fail the snapshot
        };
        let columns: Vec<Value> = provider
            .schema()
            .fields()
            .iter()
            .map(|f| {
                json!({
                    "name": f.name(),
                    "type": f.data_type().to_string(),
                    "nullable": f.is_nullable(),
                })
            })
            .collect();

        // Best-effort by contract (a kind that can't answer returns `[]`, never an error).
        let fks = source.foreign_keys(name).await.unwrap_or_default();
        for fk in &fks {
            relationships.push(json!({
                "from": format!("{name}.{}", fk.column),
                "to": format!("{}.{}", fk.ref_table, fk.ref_column),
                "kind": "foreign_key",
            }));
        }

        let rows = sample_rows(name, &provider, limit)
            .await
            .unwrap_or_else(|_| json!({ "columns": [], "values": [] }));

        out_tables.push(json!({
            "name": name,
            "columns": columns,
            "foreign_keys": fks.iter().map(fk_json).collect::<Vec<_>>(),
            "rows": rows,
            "row_limit": limit,
        }));
    }

    Ok(json!({
        "tables": out_tables,
        "relationships": relationships,
        "truncated": truncated,
    }))
}

fn fk_json(fk: &ForeignKeyMeta) -> Value {
    json!({ "column": fk.column, "ref_table": fk.ref_table, "ref_column": fk.ref_column })
}

/// Read up to `limit` rows of `table` through the engine (no SQL string — a plan-level scan +
/// limit, so a weird table name needs no quoting) and shape them `{columns, values}` with cells
/// redacted/truncated for the prompt.
async fn sample_rows(
    table: &str,
    provider: &std::sync::Arc<dyn datafusion::catalog::TableProvider>,
    limit: usize,
) -> Result<Value, String> {
    let ctx = SessionContext::new();
    let reference = TableReference::bare(table);
    ctx.register_table(reference.clone(), provider.clone())
        .map_err(|e| format!("register {table}: {e}"))?;
    let df = ctx
        .table(reference)
        .await
        .map_err(|e| format!("scan {table}: {e}"))?;
    let df = df.limit(0, Some(limit)).map_err(|e| e.to_string())?;
    let batches = df
        .collect()
        .await
        .map_err(|e| format!("read {table}: {e}"))?;
    let shaped = crate::query::shape(batches)?;

    let redact: Vec<bool> = shaped
        .columns
        .iter()
        .map(|c| {
            let lc = c.to_lowercase();
            REDACT.iter().any(|term| lc.contains(term))
        })
        .collect();
    let values: Vec<Value> = shaped
        .rows
        .into_iter()
        .map(|row| match row {
            Value::Array(cells) => Value::Array(
                cells
                    .into_iter()
                    .enumerate()
                    .map(|(i, cell)| shape_cell(cell, redact.get(i).copied().unwrap_or(false)))
                    .collect(),
            ),
            other => other,
        })
        .collect();

    Ok(json!({ "columns": shaped.columns, "values": values }))
}

/// Redact a denylisted cell (unless NULL — nullness is honest signal) and truncate a long string.
fn shape_cell(cell: Value, redact: bool) -> Value {
    if redact && !cell.is_null() {
        return Value::String(REDACTED.to_string());
    }
    match cell {
        Value::String(s) if s.chars().count() > MAX_CELL_CHARS => {
            let cut: String = s.chars().take(MAX_CELL_CHARS).collect();
            Value::String(format!("{cut}…"))
        }
        other => other,
    }
}
