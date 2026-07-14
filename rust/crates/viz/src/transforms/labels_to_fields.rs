//! The `labelsToFields` transformer — Grafana's `LabelsToFieldsTransformer` (viz grafana-parity
//! scope, tranche 2a). Options verbatim: `{ mode?: "columns"|"rows", keepLabels?, valueLabel? }`.
//!
//! **columns** (default): per frame, every label key found on any field (bounded to `keepLabels`
//! when set) becomes a new column, its value repeated per row (labels are frame-constant in the
//! Grafana model); the labels are then cleared from the carrying fields. `valueLabel` renames a
//! value field to that label's value (the label is consumed, not duplicated as a column).
//!
//! **rows**: per frame, emit one frame of `{label, value}` rows describing the labels (Grafana's
//! per-series label table).
//!
//! A frame with no labels passes through untouched.

use serde_json::{Map, Value};

use crate::frame::{Field, Frame, Frames};

/// Apply `labelsToFields` to every frame.
pub fn apply(frames: Frames, options: &Value) -> Frames {
    let mode = options
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("columns");
    let keep: Option<Vec<&str>> = options
        .get("keepLabels")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect());
    let value_label = options.get("valueLabel").and_then(Value::as_str);
    frames
        .into_iter()
        .map(|f| match mode {
            "rows" => to_rows(f),
            _ => to_columns(f, keep.as_deref(), value_label),
        })
        .collect()
}

fn to_columns(frame: Frame, keep: Option<&[&str]>, value_label: Option<&str>) -> Frame {
    // The frame's label set: first-seen order across fields (frame-constant values).
    let mut labels: Vec<(String, Value)> = Vec::new();
    for f in &frame.fields {
        for (k, v) in &f.labels {
            if !labels.iter().any(|(name, _)| name == k) {
                labels.push((k.clone(), v.clone()));
            }
        }
    }
    if labels.is_empty() {
        return frame;
    }
    let rows = frame.length;
    let ref_id = frame.ref_id.clone();
    let name = frame.name.clone();

    let mut fields: Vec<Field> = frame
        .fields
        .into_iter()
        .map(|mut f| {
            // valueLabel: a labeled value field takes that label's value as its name.
            if let Some(vl) = value_label {
                if let Some(v) = f.labels.get(vl).and_then(Value::as_str) {
                    f.name = v.to_string();
                }
            }
            f.labels = Map::new(); // consumed — they are columns now
            f
        })
        .collect();
    for (k, v) in labels {
        if k.as_str() == value_label.unwrap_or("") {
            continue; // consumed by the rename, never duplicated as a column
        }
        if keep.is_some_and(|ks| !ks.contains(&k.as_str())) {
            continue;
        }
        fields.push(Field::new(k, vec![v; rows]));
    }

    let mut out = Frame::new(fields).relen();
    out.ref_id = ref_id;
    out.name = name;
    out
}

fn to_rows(frame: Frame) -> Frame {
    let mut label_col: Vec<Value> = Vec::new();
    let mut value_col: Vec<Value> = Vec::new();
    for f in &frame.fields {
        for (k, v) in &f.labels {
            label_col.push(Value::from(k.clone()));
            value_col.push(v.clone());
        }
    }
    if label_col.is_empty() {
        return frame;
    }
    let mut out = Frame::new(vec![
        Field::new("label", label_col),
        Field::new("value", value_col),
    ]);
    out.ref_id = frame.ref_id;
    out.name = frame.name;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn labeled() -> Frames {
        let mut value = Field::new("value", vec![json!(1), json!(2)]);
        value.labels.insert("host".into(), json!("a"));
        value.labels.insert("dc".into(), json!("west"));
        vec![Frame::new(vec![
            Field::new("ts", vec![json!(10), json!(20)]),
            value,
        ])]
    }

    #[test]
    fn columns_mode_adds_a_column_per_label_and_clears_them() {
        let out = apply(labeled(), &json!({}));
        let f = &out[0];
        assert_eq!(
            f.field("host").unwrap().values,
            vec![json!("a"), json!("a")]
        );
        assert_eq!(
            f.field("dc").unwrap().values,
            vec![json!("west"), json!("west")]
        );
        assert!(f.field("value").unwrap().labels.is_empty());
    }

    #[test]
    fn keep_labels_bounds_the_columns() {
        let out = apply(labeled(), &json!({ "keepLabels": ["host"] }));
        assert!(out[0].field("host").is_some());
        assert!(out[0].field("dc").is_none());
    }

    #[test]
    fn value_label_renames_the_value_field_and_consumes_the_label() {
        let out = apply(labeled(), &json!({ "valueLabel": "host" }));
        let f = &out[0];
        assert!(
            f.field("a").is_some(),
            "value field renamed to the label value"
        );
        assert!(f.field("host").is_none(), "consumed, not duplicated");
        assert!(f.field("dc").is_some());
    }

    #[test]
    fn rows_mode_emits_label_value_rows_and_unlabeled_passes_through() {
        let out = apply(labeled(), &json!({ "mode": "rows" }));
        // Each label must stay paired with its own value. Row order is serde_json Map
        // order, which is alphabetical only while `preserve_order` is off — a workspace
        // build unifies that feature in and yields insertion order. Compare the pairs.
        let labels = &out[0].field("label").unwrap().values;
        let values = &out[0].field("value").unwrap().values;
        let mut pairs: Vec<(&str, &str)> = labels
            .iter()
            .zip(values)
            .map(|(l, v)| (l.as_str().unwrap(), v.as_str().unwrap()))
            .collect();
        pairs.sort_unstable();
        assert_eq!(pairs, vec![("dc", "west"), ("host", "a")]);
        // No labels → untouched.
        let plain = vec![Frame::new(vec![Field::new("v", vec![json!(1)])])];
        let out = apply(plain, &json!({}));
        assert_eq!(out[0].field("v").unwrap().values, vec![json!(1)]);
    }
}
