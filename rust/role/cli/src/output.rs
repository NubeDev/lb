//! Output shaping (operator-cli scope, Goals: "tables by default, `-o json` for scripting; `NO_COLOR`
//! honored") — and the DEFENSIVE shaping the scope's "output drift" risk demands: the CLI shapes over
//! **whatever the tool returned**, never assuming a shape and never inventing a field (the
//! `inbox.list`/`rules.list` envelope bug is exactly the class this defends against).
//!
//! Two formats: `Table` (human, default) and `Json` (scripting, raw round-trip). The token is never a
//! value here — output shapes a tool's JSON result, and a tool result never carries the caller's
//! bearer. `NO_COLOR` is honored by never emitting ANSI at all (the tables are plain); the flag is
//! recorded so a future colored renderer stays a one-line gate, not a scattered check.

use serde_json::Value;
use tabled::builder::Builder;
use tabled::settings::Style;

use crate::error::{CliError, CliResult};

/// The chosen output format. `Table` for a terminal, `Json` for a pipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Table,
    Json,
}

impl Format {
    /// Parse the `-o/--output` flag value. `table`/`json` only; anything else is a bad-input error so
    /// a typo fails loud instead of silently defaulting.
    pub fn parse(s: &str) -> CliResult<Format> {
        match s.to_ascii_lowercase().as_str() {
            "table" => Ok(Format::Table),
            "json" => Ok(Format::Json),
            other => Err(CliError::BadInput(format!(
                "unknown output format '{other}' (expected table|json)"
            ))),
        }
    }
}

/// Is color suppressed? `NO_COLOR` (any non-empty value) suppresses per the informal standard. The
/// CLI's tables are already plain ASCII, so this currently gates nothing visible — recorded so a later
/// colorized renderer honors it by construction.
pub fn color_suppressed() -> bool {
    std::env::var_os("NO_COLOR")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

/// Render a tool result `value` in `format`. `Json` is a pretty round-trip of the raw value (no field
/// invented, no field dropped). `Table` shapes DEFENSIVELY over the value's shape — see [`table`].
pub fn render(value: &Value, format: Format) -> CliResult<String> {
    match format {
        Format::Json => serde_json::to_string_pretty(value)
            .map_err(|e| CliError::Other(format!("serialize result: {e}"))),
        Format::Table => Ok(table(value)),
    }
}

/// Shape an arbitrary tool result into a table WITHOUT assuming its shape:
///   - an **array of objects** → one row per element, union of keys as columns (the common list shape);
///   - a **single object** → a two-column `field | value` table;
///   - an **`{ items: [...] }`** or **`{ rows: [...] }`** envelope → unwrap to the inner array (the
///     `inbox.list` shape: `{ items: [...] }`) then table it;
///   - anything else (a scalar, an array of scalars) → its pretty JSON, verbatim.
/// This is the "shape what the server sends, do not assume a shape" discipline the scope's drift risk
/// names: an unexpected shape degrades to readable JSON, never a panic and never an invented column.
pub fn table(value: &Value) -> String {
    // Unwrap the common list envelopes first (`{items|rows: [...]}`) — the typed `inbox list` returns
    // `{ items: [...] }`, and we table the items, not a one-row "items" cell.
    if let Value::Object(map) = value {
        for key in ["items", "rows", "results", "data", "reminders"] {
            if map.len() == 1 {
                if let Some(inner @ Value::Array(_)) = map.get(key) {
                    return table(inner);
                }
            }
        }
    }

    match value {
        Value::Array(rows) if rows.iter().all(|r| r.is_object()) && !rows.is_empty() => {
            array_of_objects_table(rows)
        }
        Value::Array(rows) if rows.is_empty() => "(no rows)".to_string(),
        Value::Object(map) => single_object_table(map),
        // A scalar or an array of scalars — nothing to tabulate; show it verbatim.
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    }
}

/// One row per object; columns are the union of all keys (stable order: first-seen). A missing key in
/// a given row renders empty — never a fabricated value.
fn array_of_objects_table(rows: &[Value]) -> String {
    let mut columns: Vec<String> = Vec::new();
    for row in rows {
        if let Value::Object(map) = row {
            for k in map.keys() {
                if !columns.iter().any(|c| c == k) {
                    columns.push(k.clone());
                }
            }
        }
    }
    let mut builder = Builder::default();
    builder.push_record(columns.iter().cloned());
    for row in rows {
        if let Value::Object(map) = row {
            let record: Vec<String> = columns
                .iter()
                .map(|c| map.get(c).map(cell).unwrap_or_default())
                .collect();
            builder.push_record(record);
        }
    }
    builder.build().with(Style::sharp()).to_string()
}

/// A single object as a `field | value` table.
fn single_object_table(map: &serde_json::Map<String, Value>) -> String {
    if map.is_empty() {
        return "(empty)".to_string();
    }
    let mut builder = Builder::default();
    builder.push_record(["field", "value"]);
    for (k, v) in map {
        builder.push_record([k.clone(), cell(v)]);
    }
    builder.build().with(Style::sharp()).to_string()
}

/// Render one cell value: strings bare (no quotes), everything else as compact JSON — so a table cell
/// reads cleanly but a nested object/array is still shown faithfully.
fn cell(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_round_trips_verbatim() {
        let v = json!({ "items": [{ "id": "1", "body": "hi" }] });
        let out = render(&v, Format::Json).unwrap();
        let back: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(back, v, "json output must round-trip the exact value");
    }

    #[test]
    fn table_unwraps_the_items_envelope() {
        // The inbox.list shape: `{ items: [...] }`. The table shows the items' columns, not a single
        // "items" cell — the defensive-unwrap the drift risk requires.
        let v = json!({ "items": [
            { "id": "i1", "body": "hello", "author": "user:ada" },
            { "id": "i2", "body": "world", "author": "user:bo" },
        ]});
        let t = table(&v);
        assert!(t.contains("id"), "columns come from the items: {t}");
        assert!(t.contains("author"), "{t}");
        assert!(t.contains("i1") && t.contains("i2"), "{t}");
        assert!(
            !t.contains("items"),
            "the envelope key must be unwrapped: {t}"
        );
    }

    #[test]
    fn table_of_objects_unions_keys_missing_renders_empty() {
        // Row 2 lacks `extra` — the cell is empty, never invented.
        let v = json!([
            { "id": "a", "extra": "x" },
            { "id": "b" },
        ]);
        let t = table(&v);
        assert!(t.contains("id") && t.contains("extra"), "{t}");
        assert!(t.contains('a') && t.contains('b'), "{t}");
    }

    #[test]
    fn empty_list_says_no_rows_not_an_error() {
        // Distinguishing an empty list from an error is the scope's named concern — an empty result is
        // a legible "(no rows)", not a fabricated success and not a crash.
        assert_eq!(table(&json!({ "items": [] })), "(no rows)");
        assert_eq!(table(&json!([])), "(no rows)");
    }

    #[test]
    fn single_object_is_a_field_value_table() {
        let v = json!({ "pending": 3, "sent": 10 });
        let t = table(&v);
        assert!(t.contains("field") && t.contains("value"), "{t}");
        assert!(t.contains("pending") && t.contains('3'), "{t}");
    }

    #[test]
    fn scalar_result_shows_verbatim() {
        // A tool that returns a bare value (not an object/array) is shown as-is, never forced into a table.
        assert_eq!(table(&json!("ok")), "\"ok\"");
    }

    #[test]
    fn format_parse_rejects_garbage() {
        assert!(Format::parse("table").is_ok());
        assert!(Format::parse("JSON").is_ok());
        assert!(Format::parse("yaml").is_err());
    }
}
