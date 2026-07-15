//! Export — the ways rows LEAVE a Frame: `records()` (array of row maps), `col(name)` (a plain
//! array feeding the `stats` family), `to_grid_json()` (`#{columns, rows}` — chart-ready), and
//! the string exports `to_csv_string()`/`to_json_string()` (bounded by `max_string_bytes`).
//! Every path goes through `any_value_to_json`, so NaN/Inf → `null` at ALL export boundaries
//! (scope NaN/null policy). CSV is serialized by hand (RFC-4180 quoting) — deliberately NOT via
//! polars' `csv` feature, which would also register `read_csv` in the SQL namespace and widen
//! the cage (the security pin's whole point).

use rhai::{Dynamic, Engine, EvalAltResult};
use serde_json::Value;

use crate::convert::value_to_dynamic;
use crate::json::{frame_col_json, frame_to_json};
use crate::value::{perr, rerr, Frame};

/// Register the export verbs.
pub(crate) fn register(engine: &mut Engine) {
    engine.register_fn(
        "records",
        |f: &mut Frame| -> Result<rhai::Array, Box<EvalAltResult>> {
            let rows = frame_to_json(&f.df).map_err(perr)?;
            Ok(rows.iter().map(value_to_dynamic).collect())
        },
    );
    engine.register_fn(
        "col",
        |f: &mut Frame, name: &str| -> Result<rhai::Array, Box<EvalAltResult>> {
            let values = frame_col_json(&f.df, name).map_err(perr)?;
            Ok(values.iter().map(value_to_dynamic).collect())
        },
    );
    engine.register_fn(
        "to_grid_json",
        |f: &mut Frame| -> Result<rhai::Map, Box<EvalAltResult>> {
            let rows = frame_to_json(&f.df).map_err(perr)?;
            let mut m = rhai::Map::new();
            m.insert(
                "columns".into(),
                Dynamic::from_array(
                    f.df.get_column_names()
                        .iter()
                        .map(|n| Dynamic::from(n.to_string()))
                        .collect(),
                ),
            );
            m.insert(
                "rows".into(),
                Dynamic::from_array(rows.iter().map(value_to_dynamic).collect()),
            );
            Ok(m)
        },
    );
    engine.register_fn(
        "to_json_string",
        |f: &mut Frame| -> Result<String, Box<EvalAltResult>> {
            let rows = frame_to_json(&f.df).map_err(perr)?;
            let s =
                serde_json::to_string(&rows).map_err(|e| rerr(format!("to_json_string: {e}")))?;
            f.limits.check_string(s.len()).map_err(rerr)?;
            Ok(s)
        },
    );
    engine.register_fn(
        "to_csv_string",
        |f: &mut Frame| -> Result<String, Box<EvalAltResult>> { to_csv_string(f) },
    );
}

/// Hand-rolled CSV (header + rows, RFC-4180 quoting; null → empty field). See the module doc for
/// why this is NOT polars' csv feature.
fn to_csv_string(f: &Frame) -> Result<String, Box<EvalAltResult>> {
    let names: Vec<String> =
        f.df.get_column_names()
            .iter()
            .map(|n| n.to_string())
            .collect();
    let rows = frame_to_json(&f.df).map_err(perr)?;
    let mut out = String::new();
    out.push_str(
        &names
            .iter()
            .map(|n| csv_field(&Value::String(n.clone())))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');
    for row in &rows {
        let line = names
            .iter()
            .map(|n| csv_field(row.get(n).unwrap_or(&Value::Null)))
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&line);
        out.push('\n');
        // Check as we go so a huge frame stops at the cap instead of building the whole string.
        f.limits.check_string(out.len()).map_err(rerr)?;
    }
    Ok(out)
}

/// One CSV field: null → empty; strings quoted when they contain a comma/quote/newline.
fn csv_field(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => {
            if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.clone()
            }
        }
        other => other.to_string(),
    }
}
