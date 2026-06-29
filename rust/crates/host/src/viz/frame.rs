//! Row normalization at the resolver's edge — turn a target tool's JSON result into the rows the
//! canonical [`lb_viz::Frame`] is built from (viz transformations scope, "the row↔frame adapter lives
//! at the resolver's edges"). The host MIRRORS the client's shipped `useSource.toRows` so a
//! no-transform panel resolved through `viz.query` yields the SAME rows the Phase-2 client fetch did
//! (the parity the swap depends on) — a tool may return `{samples:[…]}`, a bare array, a single
//! object, or a scalar.
//!
//! One responsibility: a tool result `Value` → `Vec<Value>` rows + the detected time field. No store,
//! no caps here (the dispatch already happened); this is pure shaping.

use serde_json::Value;

/// The keys a tabular tool result hides its rows under (mirrors the client `toRows`).
const ROW_KEYS: &[&str] = &["samples", "items", "rows", "templates", "dashboards"];

/// The canonical time-column names a row may carry (first match wins). Used to TAG the frame's time
/// field so the renderer/axis treats it as an instant — the value stays canonical epoch-ms.
const TIME_KEYS: &[&str] = &["ts", "time", "timestamp", "_time"];

/// Normalize a tool result into rows. Handles the columnar `{columns, rows}` shape (`federation.query`
/// — column-aligned arrays zipped into objects), `{samples:[…]}`/`{items}`/…, a bare array, a single
/// object, or a scalar (wrapped as `{value: scalar}`, mirroring the client).
pub fn result_to_rows(result: &Value) -> Vec<Value> {
    match result {
        Value::Null => Vec::new(),
        Value::Array(a) => a.clone(),
        Value::Object(o) => {
            // Columnar shape FIRST (`federation.query` returns `{columns:[…], rows:[[…], …]}` —
            // column-aligned arrays, NOT row-objects). Zip each row array against `columns` into an
            // object so the frame builder sees named fields. Only when `rows` is an array-of-arrays;
            // a `{rows:[{…}]}` tool (already objects) falls through to the generic ROW_KEYS path.
            if let (Some(Value::Array(columns)), Some(Value::Array(rows))) =
                (o.get("columns"), o.get("rows"))
            {
                if rows.iter().all(|r| r.is_array()) {
                    let names: Vec<String> = columns
                        .iter()
                        .enumerate()
                        .map(|(i, c)| {
                            c.as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| i.to_string())
                        })
                        .collect();
                    return rows
                        .iter()
                        .map(|row| {
                            let cells = row.as_array().cloned().unwrap_or_default();
                            let obj: serde_json::Map<String, Value> = names
                                .iter()
                                .cloned()
                                .zip(cells.into_iter().chain(std::iter::repeat(Value::Null)))
                                .collect();
                            Value::Object(obj)
                        })
                        .collect();
                }
            }
            for k in ROW_KEYS {
                if let Some(Value::Array(a)) = o.get(*k) {
                    return a.clone();
                }
            }
            vec![result.clone()]
        }
        scalar => vec![serde_json::json!({ "value": scalar })],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn federation_columnar_result_zips_into_named_rows() {
        // `federation.query` returns column-aligned arrays + a `columns` list — NOT row-objects.
        let result = json!({
            "columns": ["id", "name"],
            "rows": [["site-001", "Northside Factory"], ["site-002", "Southbank Office"]],
        });
        let rows = result_to_rows(&result);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0],
            json!({"id": "site-001", "name": "Northside Factory"})
        );
        assert_eq!(
            rows[1],
            json!({"id": "site-002", "name": "Southbank Office"})
        );
    }

    #[test]
    fn short_row_pads_missing_cells_with_null() {
        let result = json!({ "columns": ["a", "b", "c"], "rows": [[1, 2]] });
        let rows = result_to_rows(&result);
        assert_eq!(rows[0], json!({"a": 1, "b": 2, "c": null}));
    }

    #[test]
    fn object_rows_still_pass_through_unchanged() {
        // A tool already returning `{rows:[{…}]}` (objects) must NOT be re-zipped — no `columns` key.
        let result = json!({ "rows": [{"x": 1}, {"x": 2}] });
        let rows = result_to_rows(&result);
        assert_eq!(rows, vec![json!({"x": 1}), json!({"x": 2})]);
    }

    #[test]
    fn samples_shape_unchanged() {
        let result = json!({ "samples": [{"ts": 1, "value": 9}] });
        assert_eq!(result_to_rows(&result), vec![json!({"ts": 1, "value": 9})]);
    }
}

/// The time field of a row set, if any row carries a canonical time key — so the frame builder tags
/// that column `Time`. Returns the first matching key by `TIME_KEYS` priority.
pub fn detect_time_field(rows: &[Value]) -> Option<String> {
    let first = rows.iter().find_map(|r| r.as_object())?;
    TIME_KEYS
        .iter()
        .find(|k| first.contains_key(**k))
        .map(|k| k.to_string())
}
