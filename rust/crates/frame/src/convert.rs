//! rhai `Dynamic` ↔ polars value conversion — the one place the cage's dynamic values become
//! typed polars literals/rows and come back out. `NaN`/`Inf` → `null` on the way IN as well as
//! out (scope NaN/null policy: missing is `()` ↔ `null`, normalized at the frame boundary).

use polars::prelude::{lit, AnyValue, Expr, PlSmallStr, Series};
use rhai::{Dynamic, EvalAltResult};
use serde_json::Value;

use crate::value::{perr, rerr};

/// A rhai Dynamic → serde_json Value (recursive). Mirrors the cage's own converter; NaN/Inf
/// floats normalize to `null` here so a `frame(records)` built from computed floats obeys the
/// boundary policy from the first row.
pub(crate) fn dynamic_to_value(d: &Dynamic) -> Value {
    if d.is_unit() {
        Value::Null
    } else if let Ok(b) = d.as_bool() {
        Value::Bool(b)
    } else if let Ok(i) = d.as_int() {
        Value::Number(i.into())
    } else if let Ok(f) = d.as_float() {
        serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    } else if let Some(s) = d.read_lock::<String>() {
        Value::String(s.clone())
    } else if d.is_array() {
        let arr = d.clone().into_array().unwrap_or_default();
        Value::Array(arr.iter().map(dynamic_to_value).collect())
    } else if d.is_map() {
        match d.read_lock::<rhai::Map>() {
            Some(m) => {
                let mut o = serde_json::Map::new();
                for (k, v) in m.iter() {
                    o.insert(k.to_string(), dynamic_to_value(v));
                }
                Value::Object(o)
            }
            None => Value::Null,
        }
    } else {
        Value::String(d.to_string())
    }
}

/// serde_json Value → rhai Dynamic (recursive) — the export direction (`records()`, `col()`).
pub(crate) fn value_to_dynamic(v: &Value) -> Dynamic {
    match v {
        Value::Null => Dynamic::UNIT,
        Value::Bool(b) => Dynamic::from_bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from_int(i)
            } else {
                Dynamic::from_float(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => Dynamic::from(s.clone()),
        Value::Array(a) => Dynamic::from_array(a.iter().map(value_to_dynamic).collect()),
        Value::Object(o) => {
            let mut m = rhai::Map::new();
            for (k, val) in o {
                m.insert(k.as_str().into(), value_to_dynamic(val));
            }
            Dynamic::from_map(m)
        }
    }
}

/// A scalar rhai Dynamic → a polars literal `Expr` (for the filter/fill/clip verbs). Only the
/// dtypes real rows carry: int, float (NaN → null), bool, string, unit → null.
pub(crate) fn dynamic_to_lit(d: &Dynamic) -> Result<Expr, Box<EvalAltResult>> {
    if let Ok(i) = d.as_int() {
        Ok(lit(i))
    } else if let Ok(f) = d.as_float() {
        if f.is_nan() || f.is_infinite() {
            Ok(lit(polars::prelude::Null {}))
        } else {
            Ok(lit(f))
        }
    } else if let Ok(b) = d.as_bool() {
        Ok(lit(b))
    } else if let Ok(s) = d.clone().into_immutable_string() {
        // rhai carries strings as `ImmutableString`, NOT `String` — a `read_lock::<String>()`
        // never matches, so every string literal fell through to the "not a scalar" error.
        Ok(lit(s.to_string()))
    } else if d.is_unit() {
        Ok(lit(polars::prelude::Null {}))
    } else {
        Err(rerr(format!(
            "expected a scalar (int/float/bool/string), got {}",
            d.type_name()
        )))
    }
}

/// A scalar rhai Dynamic → a polars `AnyValue` (for building whole columns).
pub(crate) fn dynamic_to_any_value(d: &Dynamic) -> Result<AnyValue<'static>, Box<EvalAltResult>> {
    if d.is_unit() {
        Ok(AnyValue::Null)
    } else if let Ok(i) = d.as_int() {
        Ok(AnyValue::Int64(i))
    } else if let Ok(f) = d.as_float() {
        if f.is_nan() || f.is_infinite() {
            Ok(AnyValue::Null)
        } else {
            Ok(AnyValue::Float64(f))
        }
    } else if let Ok(b) = d.as_bool() {
        Ok(AnyValue::Boolean(b))
    } else if let Ok(s) = d.clone().into_immutable_string() {
        // rhai strings are `ImmutableString` (see `dynamic_to_lit`).
        Ok(AnyValue::StringOwned(PlSmallStr::from_str(s.as_str())))
    } else {
        Err(rerr(format!(
            "expected a scalar (int/float/bool/string), got {}",
            d.type_name()
        )))
    }
}

/// A rhai array of scalars → a polars `Series` (for `with_col_from` and `filter_in`).
pub(crate) fn array_to_series(name: &str, arr: &rhai::Array) -> Result<Series, Box<EvalAltResult>> {
    let values = arr
        .iter()
        .map(dynamic_to_any_value)
        .collect::<Result<Vec<_>, _>>()?;
    Series::from_any_values(PlSmallStr::from_str(name), &values, false).map_err(perr)
}

/// Coerce a rhai array of string-likes into `Vec<String>` (column-name lists).
pub(crate) fn string_list(arr: &rhai::Array) -> Result<Vec<String>, Box<EvalAltResult>> {
    arr.iter()
        .map(|d| {
            d.clone()
                .into_string()
                .map_err(|_| rerr("expected a string in the array"))
        })
        .collect()
}
