//! The JSON ↔ polars `DataFrame` boundary — the two halves of the catalog's `frame(records)`/
//! `f.records()` pair, plus `f.col("value") → plain array` (the bridge from a Frame to the
//! `stats::*` family, which works on plain rhai arrays).
//!
//! `()`/null → polars `null`. `NaN` → `null` at this boundary (scope NaN/null policy:
//! "NaN is normalized to null at the frame boundary"). Missing keys across rows become `null` in
//! those rows (polars' natural heterogeneous-row behavior). The eager in-frame NaN normalization
//! lands in Phase 2 with its fixture tests; the per-value path here (`any_value_to_json`) is honest
//! from the first round-trip.

use polars::frame::DataFrame;
use polars::prelude::*;
use serde_json::Value;

/// Build a polars `DataFrame` from an array of JSON row objects (the shape `Grid::records` and
/// `g.records()` produce). Phase 0's round-trip proof: this + [`frame_to_json`] are exercised before
/// any verb is written.
pub fn frame_from_json(rows: &[Value]) -> PolarsResult<DataFrame> {
    if rows.is_empty() {
        return Ok(DataFrame::empty());
    }
    let df = JsonReader::new(std::io::Cursor::new(
        serde_json::to_vec(rows).map_err(|e| PolarsError::ComputeError(e.to_string().into()))?,
    ))
    .finish()?;
    // Eager NaN → null normalization across float columns (scope NaN/null policy) lands in Phase 2
    // with its fixture tests; Phase 0 proves the JSON↔Frame round-trip is sound without it.
    Ok(df)
}

/// Serialize a polars `DataFrame` back to an array of JSON row objects (the inverse — feeds
/// `channel.post` bodies, `alert` data, and `f.records()`).
pub fn frame_to_json(df: &DataFrame) -> PolarsResult<Vec<Value>> {
    let mut buf = Vec::new();
    let mut df_mut = df.clone();
    JsonWriter::new(&mut buf)
        .with_json_format(JsonFormat::Json)
        .finish(&mut df_mut)?;
    let out: Vec<Value> = serde_json::from_slice(&buf)
        .map_err(|e| PolarsError::ComputeError(e.to_string().into()))?;
    Ok(out)
}

/// Pluck one column out as a flat `serde_json::Value` array — the catalog's `f.col("value") → plain
/// array`. `NaN`/`null` → `Value::Null` (scope NaN/null policy: missing is `()` ↔ `null`).
pub fn frame_col_json(df: &DataFrame, name: &str) -> PolarsResult<Vec<Value>> {
    let s = df.column(name)?.as_materialized_series();
    Ok(s.iter().map(|v| any_value_to_json(&v)).collect())
}

/// A polars `AnyValue` → serde_json `Value`, normalizing `NaN` → `null` (scope NaN/null policy at
/// the boundary). Phase 0 uses it for the column pluck; Phase 2 uses it across the full surface.
pub fn any_value_to_json(v: &AnyValue) -> Value {
    match v {
        AnyValue::Null => Value::Null,
        AnyValue::Boolean(b) => Value::Bool(*b),
        AnyValue::Int64(i) => Value::from(*i),
        AnyValue::Float64(f) => float_value(*f),
        AnyValue::String(s) => Value::String(s.to_string()),
        AnyValue::Int32(i) => Value::from(*i),
        AnyValue::Float32(f) => float_value(*f as f64),
        other => Value::String(format!("{other}")),
    }
}

/// A finite f64 → `Value::Number`; `NaN`/`Inf` → `Value::Null` (the boundary's missing ↔ null rule).
fn float_value(f: f64) -> Value {
    if f.is_nan() || f.is_infinite() {
        Value::Null
    } else {
        serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}
