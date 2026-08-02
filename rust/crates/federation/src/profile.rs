//! `run_profile` — the `federation.profile` engine pass (datasource-profile scope): ONE bounded
//! discovery pass over a source, computed next to the data, producing the per-column statistics that
//! every "smart" consumer currently re-derives live from a browser.
//!
//! What it replaces: a UI that wants to know "what can I chart here?" issues N× `SELECT DISTINCT …
//! LIMIT 200` plus N× `GROUP BY` range scans, one `federation.query` round trip each, uncached, on
//! every page load — ~20 s on a wide table, re-paid by every user. This pass computes the same
//! inputs in one sidecar visit and the host persists the answer, so the read path becomes a store
//! read.
//!
//! Per table: the columns and their kinds, the real foreign keys, per-text-column cardinality +
//! top-K values, per-numeric/time-column min/max + null fraction, and the grouped value ranges that
//! tell a METRIC column from a PLACE column.
//!
//! HARD BOUNDS, because this spends work on someone else's database: `max_tables` tables,
//! `max_values` values per column, [`MAX_DISTINCT_SCAN`] groups scanned before cardinality is
//! reported as capped, cells truncated, and `truncated: true` whenever something was cut.
//!
//! DISCOVERY IS NEVER SQL. Every read here goes through the provider/plan path — `list_tables`,
//! `table_provider`, `foreign_keys`, and DataFrame aggregates. It never sends an
//! `information_schema`/`pg_class` SELECT, which the engine cannot plan (only the tables a query
//! references are registered — see `debugging/datasources/discovery-via-information-schema-sql-
//! unplannable.md`).
//!
//! Rule 10: everything below is expressed over the generic `Source` trait and the Arrow schema. A
//! kind that cannot answer an aggregate contributes nulls; it never errors and is never named.

use datafusion::functions_aggregate::expr_fn::{count, max, min};
use datafusion::prelude::{col, SessionContext};
use datafusion::sql::TableReference;
use serde_json::{json, Map, Value};

use crate::pool::cached_connect;
use crate::source::ForeignKeyMeta;

/// The record shape version. Bump when the emitted JSON changes incompatibly — the host stores it on
/// the record so a consumer (and the reactor) can tell an old profile from a current one.
pub const PROFILE_VERSION: u32 = 1;

/// At most this many tables per pass (deterministic order; `truncated: true` when cut).
pub const MAX_TABLES: usize = 25;
/// At most this many distinct values retained per text column — matches the metric-cardinality
/// ceiling a consumer applies anyway, so a bigger list would be carried and then thrown away.
pub const MAX_VALUES: usize = 60;
/// At most this many groups SCANNED when counting a text column's cardinality. Beyond this the
/// column is reported as `distinct: MAX_DISTINCT_SCAN, distinct_capped: true` — honest about being a
/// floor rather than pretending to an exact count nobody asked us to pay for.
pub const MAX_DISTINCT_SCAN: usize = 200;
/// A string cell longer than this is truncated (with an ellipsis) — the size backstop.
const MAX_CELL_CHARS: usize = 256;
/// A column whose lowercased name contains one of these has its VALUES emitted as `«redacted»` — the
/// same fixed built-in denylist `federation.sample` applies, for the same reason: this record is
/// destined for agent context, so it travels further than a query result.
const REDACT: &[&str] = &["password", "secret", "token", "api_key", "apikey", "hash"];
const REDACTED: &str = "«redacted»";

/// The neutral column kinds a consumer reasons over — deliberately coarse (this is chart/SQL-shape
/// inference, not a type system) and derived from the Arrow type, never from the column NAME.
fn column_kind(dt: &datafusion::arrow::datatypes::DataType) -> &'static str {
    use datafusion::arrow::datatypes::DataType as D;
    match dt {
        D::Utf8 | D::LargeUtf8 | D::Utf8View => "text",
        D::Int8
        | D::Int16
        | D::Int32
        | D::Int64
        | D::UInt8
        | D::UInt16
        | D::UInt32
        | D::UInt64
        | D::Float16
        | D::Float32
        | D::Float64
        | D::Decimal128(_, _)
        | D::Decimal256(_, _) => "number",
        D::Timestamp(_, _) | D::Date32 | D::Date64 | D::Time32(_) | D::Time64(_) => "time",
        D::Dictionary(_, inner) => column_kind(inner),
        _ => "other",
    }
}

/// The caller-tunable bounds (`BootConfig::profile` supplies them host-side). Clamped to the
/// compile-time ceilings above — a config value can only ever make a pass CHEAPER.
#[derive(Debug, Clone, Copy)]
pub struct ProfileBounds {
    pub max_tables: usize,
    pub max_values: usize,
}

impl Default for ProfileBounds {
    fn default() -> Self {
        Self {
            max_tables: MAX_TABLES,
            max_values: MAX_VALUES,
        }
    }
}

impl ProfileBounds {
    fn clamped(self) -> Self {
        Self {
            max_tables: self.max_tables.clamp(1, MAX_TABLES),
            max_values: self.max_values.clamp(1, MAX_VALUES),
        }
    }
}

/// Profile the `kind` source at `dsn`: `{tables[], truncated, version}`. `tables` filters to the
/// named tables when present. Per-table reads are best-effort — one unreadable table is skipped, not
/// a failed pass (the `run_sample` stance). The DSN lives only inside the pool and never appears in
/// the result.
pub async fn run_profile(
    kind: &str,
    dsn: &str,
    tables: Option<Vec<String>>,
    bounds: ProfileBounds,
) -> Result<Value, String> {
    let bounds = bounds.clamped();
    let source = cached_connect(kind, dsn).await.map_err(|e| e.to_string())?;

    let metas = source.list_tables().await.map_err(|e| e.to_string())?;
    // Sorted, so truncation is deterministic and a re-profile of an unchanged source upserts a
    // byte-identical record (the idempotence the scope requires).
    let mut metas: Vec<_> = metas.into_iter().collect();
    metas.sort_by(|a, b| a.name.cmp(&b.name));
    if let Some(filter) = &tables {
        metas.retain(|m| filter.iter().any(|f| f == &m.name));
    }
    let mut truncated = metas.len() > bounds.max_tables;
    metas.truncate(bounds.max_tables);

    let mut out_tables = Vec::new();
    for meta in &metas {
        let Ok(provider) = source
            .table_provider(&TableReference::bare(meta.name.clone()))
            .await
        else {
            continue; // best-effort: one unreadable table must not fail the pass
        };

        let schema = provider.schema();
        let mut columns: Vec<Map<String, Value>> = schema
            .fields()
            .iter()
            .map(|f| {
                let mut m = Map::new();
                m.insert("name".into(), json!(f.name()));
                m.insert("type".into(), json!(f.data_type().to_string()));
                m.insert("kind".into(), json!(column_kind(f.data_type())));
                m
            })
            .collect();

        // ONE registered context per table, reused by every aggregate below — registering the
        // provider per query would re-plan the federation adaptor for each column.
        let ctx = SessionContext::new();
        let reference = TableReference::bare(meta.name.clone());
        if ctx.register_table(reference.clone(), provider.clone()).is_err() {
            continue;
        }

        let text: Vec<String> = columns
            .iter()
            .filter(|c| c["kind"] == json!("text"))
            .map(|c| c["name"].as_str().unwrap_or_default().to_string())
            .collect();
        let ranged: Vec<String> = columns
            .iter()
            .filter(|c| c["kind"] == json!("number") || c["kind"] == json!("time"))
            .map(|c| c["name"].as_str().unwrap_or_default().to_string())
            .collect();

        let stats = min_max_stats(&ctx, &reference, &ranged).await;
        let mut per_column: Map<String, Value> = Map::new();
        for name in &text {
            if let Some(v) = distinct_stats(&ctx, &reference, name, bounds.max_values).await {
                per_column.insert(name.clone(), v);
            }
        }

        // A capped cardinality count means the pass stopped short of the truth somewhere — the same
        // thing `truncated` says about tables. Read it BEFORE the drain below empties `per_column`.
        if per_column
            .values()
            .any(|v| v.get("distinct_capped").is_some())
        {
            truncated = true;
        }

        for c in &mut columns {
            let name = c["name"].as_str().unwrap_or_default().to_string();
            if let Some(Value::Object(o)) = per_column.remove(&name) {
                for (k, v) in o {
                    c.insert(k, v);
                }
            }
            if let Some(Value::Object(o)) = stats.get(&name).cloned() {
                for (k, v) in o {
                    c.insert(k, v);
                }
            }
        }

        // Best-effort by contract (a kind that can't answer returns `[]`, never an error).
        let fks = source.foreign_keys(&meta.name).await.unwrap_or_default();

        // The GROUPED RANGES — the signal that separates a metric column from a place column, and
        // the single most expensive thing a consumer used to compute from the browser. Only
        // meaningful when the table has exactly ONE numeric column: that is the long/EAV signature
        // (a wide table has one column per metric and nothing to separate). Same rule the consumer's
        // detector applies, computed here instead of over N round trips.
        let numeric_names: Vec<String> = columns
            .iter()
            .filter(|c| c["kind"] == json!("number"))
            .filter_map(|c| c["name"].as_str().map(str::to_string))
            .collect();
        let mut group_ranges = Map::new();
        if numeric_names.len() == 1 {
            let value_column = &numeric_names[0];
            for name in &text {
                if let Some(rows) =
                    range_stats(&ctx, &reference, name, value_column, bounds.max_values).await
                {
                    if !rows.is_empty() {
                        group_ranges.insert(name.clone(), Value::Array(rows));
                    }
                }
            }
        }

        let mut table = Map::new();
        table.insert("name".into(), json!(meta.name));
        // Catalog-based row estimate where the kind exposes one (Postgres `reltuples`); OMITTED
        // otherwise. Never `COUNT(*)` — an exact count on an unindexed table is exactly the
        // unbounded cost this pass exists to avoid.
        if let Some(rows) = meta.rows {
            table.insert("row_estimate".into(), json!(rows));
        }
        table.insert("columns".into(), Value::Array(columns.into_iter().map(Value::Object).collect()));
        table.insert(
            "foreign_keys".into(),
            Value::Array(fks.iter().map(fk_json).collect()),
        );
        if !group_ranges.is_empty() {
            table.insert("group_ranges".into(), Value::Object(group_ranges));
        }
        out_tables.push(Value::Object(table));
    }

    Ok(json!({
        "version": PROFILE_VERSION,
        "tables": out_tables,
        "truncated": truncated,
    }))
}

fn fk_json(fk: &ForeignKeyMeta) -> Value {
    json!({ "column": fk.column, "ref_table": fk.ref_table, "ref_column": fk.ref_column })
}

/// `MIN`/`MAX`/non-null `COUNT` for every numeric/time column of the table, plus the table's total
/// row count — ONE aggregate query for the whole table rather than one per column. Best-effort: a
/// source that cannot plan it contributes nothing and the columns simply carry no range.
async fn min_max_stats(
    ctx: &SessionContext,
    reference: &TableReference,
    names: &[String],
) -> Map<String, Value> {
    let mut out = Map::new();
    if names.is_empty() {
        return out;
    }
    let Ok(df) = ctx.table(reference.clone()).await else {
        return out;
    };
    let mut aggr = vec![count(datafusion::prelude::lit(1)).alias("__rows")];
    for (i, n) in names.iter().enumerate() {
        aggr.push(min(col(quoted(n))).alias(format!("__min{i}")));
        aggr.push(max(col(quoted(n))).alias(format!("__max{i}")));
        aggr.push(count(col(quoted(n))).alias(format!("__cnt{i}")));
    }
    let Ok(df) = df.aggregate(vec![], aggr) else {
        return out;
    };
    let Ok(batches) = df.collect().await else {
        return out;
    };
    let Ok(shaped) = crate::query::shape(batches) else {
        return out;
    };
    let Some(Value::Array(row)) = shaped.rows.first().cloned() else {
        return out;
    };
    let at = |label: &str| -> Value {
        shaped
            .columns
            .iter()
            .position(|c| c == label)
            .and_then(|i| row.get(i).cloned())
            .unwrap_or(Value::Null)
    };
    let total = at("__rows").as_f64().unwrap_or(0.0);
    for (i, n) in names.iter().enumerate() {
        let mut m = Map::new();
        m.insert("min".into(), at(&format!("__min{i}")));
        m.insert("max".into(), at(&format!("__max{i}")));
        if total > 0.0 {
            let non_null = at(&format!("__cnt{i}")).as_f64().unwrap_or(0.0);
            let frac = ((1.0 - non_null / total) * 10_000.0).round() / 10_000.0;
            m.insert("null_frac".into(), json!(frac));
        }
        out.insert(n.clone(), Value::Object(m));
    }
    out
}

/// Cardinality + top-K values for one text column: `GROUP BY col` counted, scanned to at most
/// [`MAX_DISTINCT_SCAN`] groups. Ordered by frequency (then value) so the record is stable across
/// re-profiles — the idempotence the reactor depends on.
async fn distinct_stats(
    ctx: &SessionContext,
    reference: &TableReference,
    name: &str,
    max_values: usize,
) -> Option<Value> {
    let df = ctx.table(reference.clone()).await.ok()?;
    let df = df
        .aggregate(
            vec![col(quoted(name))],
            vec![count(datafusion::prelude::lit(1)).alias("__n")],
        )
        .ok()?
        .limit(0, Some(MAX_DISTINCT_SCAN))
        .ok()?;
    let shaped = crate::query::shape(df.collect().await.ok()?).ok()?;
    let n_at = shaped.columns.iter().position(|c| c == "__n")?;
    let v_at = (0..shaped.columns.len()).find(|i| *i != n_at)?;

    let mut rows: Vec<(f64, Value)> = shaped
        .rows
        .iter()
        .filter_map(|r| match r {
            Value::Array(cells) => {
                let v = cells.get(v_at)?.clone();
                if v.is_null() {
                    return None; // NULL is not a distinct VALUE; null_frac carries that signal
                }
                Some((cells.get(n_at)?.as_f64().unwrap_or(0.0), v))
            }
            _ => None,
        })
        .collect();
    rows.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.to_string().cmp(&b.1.to_string()))
    });

    let distinct = rows.len();
    let capped = distinct >= MAX_DISTINCT_SCAN;
    let redact = is_redacted(name);
    let values: Vec<Value> = rows
        .into_iter()
        .take(max_values)
        .map(|(_, v)| shape_cell(v, redact))
        .collect();

    let mut m = Map::new();
    m.insert("distinct".into(), json!(distinct));
    if capped {
        // An honest floor, not a guess: "at least this many, we stopped counting".
        m.insert("distinct_capped".into(), json!(true));
    }
    m.insert("values".into(), Value::Array(values));
    Some(Value::Object(m))
}

/// The per-group `[min, max]` span of `value_column` grouped by `name` — the metric-vs-place signal.
/// Bounded to `max_values` groups; ordered by group so re-profiles are stable.
async fn range_stats(
    ctx: &SessionContext,
    reference: &TableReference,
    name: &str,
    value_column: &str,
    max_values: usize,
) -> Option<Vec<Value>> {
    let df = ctx.table(reference.clone()).await.ok()?;
    let df = df
        .aggregate(
            vec![col(quoted(name))],
            vec![
                min(col(quoted(value_column))).alias("__lo"),
                max(col(quoted(value_column))).alias("__hi"),
            ],
        )
        .ok()?
        .limit(0, Some(max_values))
        .ok()?;
    let shaped = crate::query::shape(df.collect().await.ok()?).ok()?;
    let lo_at = shaped.columns.iter().position(|c| c == "__lo")?;
    let hi_at = shaped.columns.iter().position(|c| c == "__hi")?;
    let g_at = (0..shaped.columns.len()).find(|i| *i != lo_at && *i != hi_at)?;

    let redact = is_redacted(name);
    let mut out: Vec<(String, Value)> = shaped
        .rows
        .iter()
        .filter_map(|r| match r {
            Value::Array(cells) => {
                let g = cells.get(g_at)?.clone();
                if g.is_null() {
                    return None;
                }
                let g = shape_cell(g, redact);
                let key = g.as_str().unwrap_or_default().to_string();
                Some((
                    key,
                    json!({ "group": g, "lo": cells.get(lo_at)?, "hi": cells.get(hi_at)? }),
                ))
            }
            _ => None,
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Some(out.into_iter().map(|(_, v)| v).collect())
}

/// True when a column's VALUES must not travel (the fixed built-in denylist). The column itself,
/// its kind and its cardinality still appear — the shape is not the secret, the contents are.
fn is_redacted(name: &str) -> bool {
    let lc = name.to_lowercase();
    REDACT.iter().any(|term| lc.contains(term))
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

/// A column reference that survives a name needing quoting. `col()` parses its argument as an
/// identifier path, so a column called `my col` (or one containing a dot) would otherwise be read as
/// a qualified reference to a table that does not exist.
fn quoted(name: &str) -> datafusion::common::Column {
    datafusion::common::Column::new_unqualified(name)
}
