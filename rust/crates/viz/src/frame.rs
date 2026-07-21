//! The canonical columnar **Frame** (viz transformations scope, "The frame shape"). Field-oriented
//! (Grafana's `DataFrame`) so transforms port near-1:1: a `Frame` is `fields[]`, each `Field` a named
//! typed column of canonical values. Values are **canonical** (UTC epoch-ms instants, SI/base units,
//! locale-neutral) — `format.*` localizes at render, never here.
//!
//! One responsibility: the Frame/Field types + the row↔frame adapter at the lib's edges. A tool result
//! is a list of JSON row objects (`store.query`/`series.read`/`federation.query` all return rows); the
//! resolver turns rows into a Frame on the way in and back into rows on the way out so the shipped
//! renderers (which consume rows) are unchanged.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A field's canonical type (Grafana `FieldType` subset we carry). `Other` covers anything not
/// number/string/time/boolean — never guessed into a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Number,
    String,
    Time,
    Boolean,
    Other,
}

impl FieldType {
    /// Infer a column's type from its first non-null value (Grafana's `guessFieldTypeForField`
    /// analog). Conservative: a bare number → `Number`, a bool → `Boolean`, else `String`/`Other`.
    /// A `time`-looking field is only ever typed `Time` when the *frame builder* names it so (the
    /// resolver tags the time column) — inference never promotes a number to a time.
    pub fn infer(values: &[Value]) -> FieldType {
        for v in values {
            match v {
                Value::Null => continue,
                Value::Number(_) => return FieldType::Number,
                Value::Bool(_) => return FieldType::Boolean,
                Value::String(_) => return FieldType::String,
                _ => return FieldType::Other,
            }
        }
        FieldType::Other
    }
}

/// One columnar field — a named, typed column of canonical values. `labels` carry Grafana series
/// labels (e.g. `{host:"a"}`) for multi-series frames; empty for a plain column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: FieldType,
    pub values: Vec<Value>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub labels: Map<String, Value>,
}

impl Field {
    /// A field from a name + canonical values, type inferred.
    pub fn new(name: impl Into<String>, values: Vec<Value>) -> Self {
        let ty = FieldType::infer(&values);
        Field {
            name: name.into(),
            ty,
            values,
            labels: Map::new(),
        }
    }

    /// A field with an explicit type (the resolver tags a time column; a transform sets a derived type).
    pub fn typed(name: impl Into<String>, ty: FieldType, values: Vec<Value>) -> Self {
        Field {
            name: name.into(),
            ty,
            values,
            labels: Map::new(),
        }
    }

    /// This field's value at `row`, or `Null` past the end (a ragged frame reads as null, never panics).
    pub fn at(&self, row: usize) -> Value {
        self.values.get(row).cloned().unwrap_or(Value::Null)
    }

    /// The numeric value at `row`, or `None` for a non-numeric/absent cell (honest — never a 0).
    pub fn num_at(&self, row: usize) -> Option<f64> {
        self.values.get(row).and_then(Value::as_f64)
    }
}

/// Why a frame is (or isn't) blank — per-target diagnostic (query-diagnostics scope). The resolver
/// distinguishes four outcomes that all otherwise reach the client as the same `{fields:[],length:0}`:
/// a query `ok` with rows, an `empty` success (ran, 0 rows), a `denied` target (opaque by design), and
/// an `error` (the downstream tool's own message — the author's bad SQL/args). Plain data — no store
/// or bus reach — so `lb-viz` stays pure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameState {
    Ok,
    Empty,
    Denied,
    Error,
}

/// A frame's resolution status. `message` is present ONLY for `error` (the downstream tool's text) —
/// `denied` carries **no** message by construction (the deny-opacity contract: an unauthorized caller
/// learns nothing, not even that a message exists), and `ok`/`empty` need none (the UI writes its own
/// "0 rows for <range>").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameStatus {
    pub state: FrameState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl FrameStatus {
    /// A target that produced ≥1 row.
    pub fn ok() -> Self {
        FrameStatus {
            state: FrameState::Ok,
            message: None,
        }
    }

    /// A target that ran and matched 0 rows (a correct-but-empty range).
    pub fn empty() -> Self {
        FrameStatus {
            state: FrameState::Empty,
            message: None,
        }
    }

    /// A denied/not-found target — opaque, never a message (never reveals a gate or tool existence).
    pub fn denied() -> Self {
        FrameStatus {
            state: FrameState::Denied,
            message: None,
        }
    }

    /// A target error carrying the downstream tool's own message (the caller's bad SQL/args echoed
    /// back to the same caller — safe and the whole point).
    pub fn error(message: impl Into<String>) -> Self {
        FrameStatus {
            state: FrameState::Error,
            message: Some(message.into()),
        }
    }
}

/// A canonical columnar frame. `refId` ties it back to the target that produced it (A, B, …) so
/// transforms (`joinByField`, `merge`) and field overrides can reference it. `length` is the row count
/// (the longest field; a ragged frame reads short fields as null). `status` is the per-target
/// resolution diagnostic (query-diagnostics scope) — `#[serde(default)]` + skip-when-absent so a
/// legacy/frames-in frame that omits it deserializes as `None` (treated as `ok`) and a client that
/// ignores it renders exactly as before.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Frame {
    #[serde(default, rename = "refId", skip_serializing_if = "String::is_empty")]
    pub ref_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub fields: Vec<Field>,
    pub length: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<FrameStatus>,
}

impl Frame {
    /// Build a frame from explicit fields, computing `length` as the longest field.
    pub fn new(fields: Vec<Field>) -> Self {
        let length = fields.iter().map(|f| f.values.len()).max().unwrap_or(0);
        Frame {
            ref_id: String::new(),
            name: String::new(),
            fields,
            length,
            status: None,
        }
    }

    /// Recompute `length` after a structural edit (a transform that adds/drops rows). Call at the end
    /// of any transform that changes row counts so downstream steps see a consistent frame.
    pub fn relen(mut self) -> Self {
        self.length = self
            .fields
            .iter()
            .map(|f| f.values.len())
            .max()
            .unwrap_or(0);
        self
    }

    /// Cap the frame to at most `max` rows — truncate every field's values and `length`. Used by the
    /// debug/stepwise view to keep an intermediate snapshot within the per-frame budget (viz
    /// transformations scope, "the frame budget is the whole game").
    pub fn truncate(&mut self, max: usize) {
        if self.length <= max {
            return;
        }
        for f in &mut self.fields {
            f.values.truncate(max);
        }
        self.length = max;
    }

    /// Find a field by name (first match — Grafana's `byName`).
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Find a field's index by name.
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f.name == name)
    }

    /// **Rows → Frame** (the inbound adapter). A tool returns rows (`Vec<{col: value}>`); we pivot to
    /// columns. The column set is the UNION of all row keys (in first-seen order so the shape is
    /// stable); a row missing a key contributes `Null` (a ragged result is honest, not dropped). Each
    /// field's type is inferred from its values; `time_field`, when present and matched, is typed
    /// `Time` (canonical epoch-ms is still a number on the wire — the type tags intent for the
    /// renderer/axis). `ref_id` ties the frame to its target.
    pub fn from_rows(ref_id: &str, rows: &[Value], time_field: Option<&str>) -> Frame {
        let mut order: Vec<String> = Vec::new();
        for row in rows {
            if let Value::Object(map) = row {
                for k in map.keys() {
                    if !order.iter().any(|o| o == k) {
                        order.push(k.clone());
                    }
                }
            }
        }
        let mut fields: Vec<Field> = Vec::with_capacity(order.len());
        for key in &order {
            let values: Vec<Value> = rows
                .iter()
                .map(|row| row.get(key).cloned().unwrap_or(Value::Null))
                .collect();
            let mut field = Field::new(key.clone(), values);
            if time_field == Some(key.as_str()) {
                field.ty = FieldType::Time;
            }
            fields.push(field);
        }
        let mut frame = Frame::new(fields);
        frame.ref_id = ref_id.to_string();
        frame.length = rows.len();
        frame
    }

    /// **Frame → Rows** (the outbound adapter). Pivot columns back to row objects so the shipped
    /// row-consuming renderers are unchanged. Row `i` is `{ field.name: field.values[i] }` over every
    /// field; a short field contributes `Null`.
    pub fn to_rows(&self) -> Vec<Value> {
        (0..self.length)
            .map(|i| {
                let mut map = Map::new();
                for f in &self.fields {
                    map.insert(f.name.clone(), f.at(i));
                }
                Value::Object(map)
            })
            .collect()
    }
}

/// The pipeline I/O — many frames in, many frames out (a join collapses N→1, a partition would expand).
pub type Frames = Vec<Frame>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // query-diagnostics scope: a `status`-less frame (legacy / frames-in) deserializes as `None` and,
    // re-serialized, emits NO `status` key — an old client and a responder stub are unaffected.
    #[test]
    fn frame_status_defaults_absent_and_round_trips() {
        let legacy = json!({ "refId": "A", "fields": [], "length": 0 });
        let f: Frame = serde_json::from_value(legacy).unwrap();
        assert_eq!(f.status, None, "absent status → None (legacy/ok)");
        let back = serde_json::to_value(&f).unwrap();
        assert!(
            back.get("status").is_none(),
            "None status is not serialized"
        );
    }

    // `error` carries its message; `denied`/`empty`/`ok` do not (denied opacity + no noise).
    #[test]
    fn frame_status_error_carries_message_others_do_not() {
        let mut f = Frame::new(vec![]);
        f.status = Some(FrameStatus::error("Schema error: No field named x"));
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!(v["status"]["state"], json!("error"));
        assert_eq!(
            v["status"]["message"],
            json!("Schema error: No field named x")
        );

        for (s, state) in [
            (FrameStatus::ok(), "ok"),
            (FrameStatus::empty(), "empty"),
            (FrameStatus::denied(), "denied"),
        ] {
            let mut f = Frame::new(vec![]);
            f.status = Some(s);
            let v = serde_json::to_value(&f).unwrap();
            assert_eq!(
                v["status"],
                json!({ "state": state }),
                "{state} has no message"
            );
        }

        // Round-trip preserves the variant + message.
        let mut f = Frame::new(vec![]);
        f.status = Some(FrameStatus::error("boom"));
        let back: Frame = serde_json::from_value(serde_json::to_value(&f).unwrap()).unwrap();
        assert_eq!(back.status, Some(FrameStatus::error("boom")));
    }
}
