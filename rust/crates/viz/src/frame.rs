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

/// A canonical columnar frame. `refId` ties it back to the target that produced it (A, B, …) so
/// transforms (`joinByField`, `merge`) and field overrides can reference it. `length` is the row count
/// (the longest field; a ragged frame reads short fields as null).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Frame {
    #[serde(default, rename = "refId", skip_serializing_if = "String::is_empty")]
    pub ref_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub fields: Vec<Field>,
    pub length: usize,
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
