//! Arrow → JSON for the Postgres direct read path: one `RecordBatch` column at a time, typed array
//! access rather than a per-cell Postgres OID dispatch. Split out of `postgres.rs`, which owns the
//! pool and the `Source` impl — rendering a cell is its own responsibility (FILE-LAYOUT §9).

use arrow::array::*;
use arrow::datatypes::{DataType, TimeUnit};
use arrow::record_batch::RecordBatch;

/// Convert Arrow RecordBatches to column-aligned `(columns, rows)` by iterating each
/// column's typed Arrow array — no JSON text intermediate, no per-cell Postgres OID dispatch.
pub(super) fn batches_to_column_rows(
    batches: &[RecordBatch],
) -> (Vec<String>, Vec<serde_json::Value>) {
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
