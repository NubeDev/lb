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

/// Normalize a tool result into rows. Handles `{samples:[…]}`/`{items}`/…, a bare array, a single
/// object, or a scalar (wrapped as `{value: scalar}`, mirroring the client).
pub fn result_to_rows(result: &Value) -> Vec<Value> {
    match result {
        Value::Null => Vec::new(),
        Value::Array(a) => a.clone(),
        Value::Object(o) => {
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

/// The time field of a row set, if any row carries a canonical time key — so the frame builder tags
/// that column `Time`. Returns the first matching key by `TIME_KEYS` priority.
pub fn detect_time_field(rows: &[Value]) -> Option<String> {
    let first = rows.iter().find_map(|r| r.as_object())?;
    TIME_KEYS
        .iter()
        .find(|k| first.contains_key(**k))
        .map(|k| k.to_string())
}
