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

/// The keys a tabular tool result hides its rows under (mirrors the client `toRows`). A list verb that
/// wraps its rows under its own plural (`reminder.list` → `{reminders:[…]}`) is added here so a
/// `rich_result` table `source`-d at that verb unwraps to N rows instead of one JSON-blob row (the
/// channel-rich-responses reminders tenant — keep in lock-step with the client mirror in `useSource.ts`).
const ROW_KEYS: &[&str] = &[
    "samples",
    "buckets",
    "items",
    "rows",
    "templates",
    "dashboards",
    "reminders",
];

/// The canonical time-column names a row may carry (first match wins). Used to TAG the frame's time
/// field so the renderer/axis treats it as an instant — the value stays canonical epoch-ms. `t` is the
/// bucket-frame time key (`series.read mode:"buckets"` → `{t,min,max,avg,last,count}`); it is LAST so a
/// row that also carries a `ts`/`time` column tags that instead (a bucket row has only `t`).
const TIME_KEYS: &[&str] = &["ts", "time", "timestamp", "_time", "t"];

/// Normalize a tool result into rows. Handles the columnar `{columns, rows}` shape (`federation.query`
/// — column-aligned arrays zipped into objects), `{samples:[…]}`/`{items}`/…, a bare array, a single
/// object, or a scalar (wrapped as `{value: scalar}`, mirroring the client).
pub fn result_to_rows(result: &Value) -> Vec<Value> {
    // A `rules.run`/`rules.eval` result is a `RunResult` envelope (`{output, findings, log, ms, ai}`)
    // whose `output` is a `RuleOutput` (`{kind:"scalar", value}` / `{kind:"grid", columns, rows}`).
    // Unwrap it FIRST — generically by shape (a `kind`-discriminated object), never by tool id — so a
    // panel bound to a rule renders the rule's N rows, not one JSON-blob row (rules-for-widgets-scope
    // slice 1, layer 2). A tool that happens to return a plain `{output: […]}` without `kind` falls
    // through untouched.
    if let Some(rows) = unwrap_rule_envelope(result) {
        return rows;
    }
    match result {
        Value::Null => Vec::new(),
        Value::Array(a) => a.clone(),
        Value::Object(o) => {
            // Columnar shape FIRST (`federation.query` returns `{columns:[…], rows:[[…], …]}` —
            // column-aligned arrays, NOT row-objects). Zip each row array against `columns` into an
            // object so the frame builder sees named fields. Only when `rows` is an array-of-arrays;
            // a `{rows:[{…}]}` tool (already objects) falls through to the generic ROW_KEYS path.
            if let Some(zipped) = columnar_rows(o) {
                return zipped;
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

    // Regression (debugging/reminders/reminder-list-not-unwrapped-to-table-rows.md): `reminder.list`
    // returns `{reminders:[…]}`; before `reminders` was added to ROW_KEYS this unwrapped to ONE
    // JSON-blob row, so the channel `/reminders` table showed one cell instead of N per-reminder rows
    // and a row control's `${id}` bound nothing.
    // rules-for-widgets-scope slice 1, layer 2: a `rules.run` result unwraps by shape to the rule's rows.
    #[test]
    fn rule_run_result_scalar_array_unwraps_to_rows() {
        // The full RunResult envelope with a scalar-array output (an array of row maps).
        let result = json!({
            "output": { "kind": "scalar", "value": [{"h": 0, "v": 10}, {"h": 1, "v": 20}] },
            "findings": [], "log": [], "ms": 0,
        });
        assert_eq!(
            result_to_rows(&result),
            vec![json!({"h": 0, "v": 10}), json!({"h": 1, "v": 20})],
            "RunResult unwraps to the rule's N rows, not one blob row"
        );
    }

    #[test]
    fn rule_output_grid_unwraps_via_columnar_path() {
        // A bare RuleOutput of kind grid — column-aligned arrays zipped into named rows.
        let result = json!({
            "kind": "grid",
            "columns": ["building", "kwh"],
            "rows": [["north", 100], ["south", 200]],
        });
        assert_eq!(
            result_to_rows(&result),
            vec![
                json!({"building": "north", "kwh": 100}),
                json!({"building": "south", "kwh": 200})
            ],
            "grid output zips through the columnar path"
        );
    }

    #[test]
    fn rule_output_scalar_non_array_is_one_value_row() {
        // A scalar that is not an array renders as an honest single `{value}` row.
        let result = json!({ "output": { "kind": "scalar", "value": 42 }, "findings": [], "log": [], "ms": 0 });
        assert_eq!(result_to_rows(&result), vec![json!({ "value": 42 })]);
    }

    #[test]
    fn rule_output_findings_kind_is_empty() {
        // findings/nothing carry no chart rows.
        let result =
            json!({ "output": { "kind": "findings" }, "findings": [{"x": 1}], "log": [], "ms": 0 });
        assert!(
            result_to_rows(&result).is_empty(),
            "findings output → no chart rows"
        );
    }

    #[test]
    fn reminders_shape_unwraps_to_n_rows() {
        let result = json!({ "reminders": [{"id": "r1"}, {"id": "r2"}] });
        assert_eq!(
            result_to_rows(&result),
            vec![json!({"id": "r1"}), json!({"id": "r2"})],
            "reminder.list unwraps to N reminder rows, not one blob row"
        );
    }
}

/// The columnar `{columns:[…], rows:[[…], …]}` shape (`federation.query` / a rule's `grid` output):
/// column-aligned arrays zipped into named row objects. `Some(rows)` only when `rows` is an array of
/// arrays (a `{rows:[{…}]}` of objects is already row-shaped and returns `None` so the generic path
/// handles it). Short rows pad with `null`; an unnamed column falls back to its index.
fn columnar_rows(o: &serde_json::Map<String, Value>) -> Option<Vec<Value>> {
    let (Some(Value::Array(columns)), Some(Value::Array(rows))) = (o.get("columns"), o.get("rows"))
    else {
        return None;
    };
    if !rows.iter().all(|r| r.is_array()) {
        return None;
    }
    let names: Vec<String> = columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            c.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| i.to_string())
        })
        .collect();
    Some(
        rows.iter()
            .map(|row| {
                let cells = row.as_array().cloned().unwrap_or_default();
                let obj: serde_json::Map<String, Value> = names
                    .iter()
                    .cloned()
                    .zip(cells.into_iter().chain(std::iter::repeat(Value::Null)))
                    .collect();
                Value::Object(obj)
            })
            .collect(),
    )
}

/// Unwrap a rules result envelope by SHAPE (never by tool id — the viz plane treats `rules.run` as an
/// opaque tool, CLAUDE §10). Two nested envelopes, both `kind`/key-discriminated:
///   - a full `RunResult` `{output, findings, log, ms, …}` → recurse into `output`;
///   - a `RuleOutput` `{kind:"scalar", value}` → the value; `{kind:"grid", columns, rows}` → the grid
///     object (the existing columnar path zips it); `{kind:"findings"|"nothing"}` → empty (no rows).
/// Returns `None` for anything that is not one of these documented shapes, so every other tool result
/// flows through the normal `result_to_rows` matching untouched — and `Some(rows)` (already normalized)
/// when it IS a rules envelope. Grid is dispatched to the shared columnar zip; scalar recurses on the
/// value; findings/nothing are honestly empty.
fn unwrap_rule_envelope(result: &Value) -> Option<Vec<Value>> {
    let o = result.as_object()?;
    // Layer A: a full RunResult carries `output` alongside `findings`/`log`/`ms`. Recurse into output.
    if let Some(output) = o.get("output") {
        if o.contains_key("findings") || o.contains_key("log") || o.contains_key("ms") {
            return Some(result_to_rows(output));
        }
    }
    // Layer B: a bare RuleOutput, discriminated by `kind`.
    match o.get("kind").and_then(Value::as_str) {
        // A scalar `value` is usually the array of row maps; recurse so an array unwraps to N rows and a
        // non-array renders as an honest single `{value}` row (the same shaping every other path uses).
        Some("scalar") => Some(result_to_rows(o.get("value").unwrap_or(&Value::Null))),
        // Route the grid's `{columns, rows}` straight into the shared columnar zip (NOT back through the
        // envelope check — the `kind:"grid"` key would re-match and recurse forever).
        Some("grid") => Some(columnar_rows(o).unwrap_or_default()),
        // findings/nothing carry no chart rows (findings are the insights plane's food, not chart rows).
        Some("findings") | Some("nothing") => Some(Vec::new()),
        _ => None,
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
