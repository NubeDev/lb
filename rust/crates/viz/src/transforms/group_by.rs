//! The `groupBy` transformer — Grafana's `GroupByTransformer` (viz transformations scope). Groups
//! rows by the tuple of every field marked `groupby`, then emits one row per group: the groupby
//! field values + one `{field} ({agg})` column per aggregation (via the shared `reduce_field`).
//! Option shape verbatim: `fields: Record<string, {aggregations: ReducerID[], operation}>`. One
//! responsibility: grouped aggregation → ONE frame. Honest: an empty/non-numeric group reduces to
//! `Null`, never a fabricated 0; with no groupby field the frames pass through (Grafana needs ≥1).

use serde_json::Value;

use crate::frame::{Field, Frame, Frames};
use crate::reducer::reduce_field;

/// One aggregate request: the source field name + the reducer ids to apply.
struct Agg {
    field: String,
    calcs: Vec<String>,
}

/// Apply `groupBy`. Operates on the FIRST frame (Grafana groups a single series set); other frames
/// pass through unchanged after the grouped frame. With no `groupby` field configured, pass through.
pub fn apply(frames: Frames, options: &Value) -> Frames {
    let config = match options.get("fields").and_then(Value::as_object) {
        Some(c) => c,
        None => return frames,
    };
    let mut group_by: Vec<String> = Vec::new();
    let mut aggs: Vec<Agg> = Vec::new();
    for (name, spec) in config {
        match spec.get("operation").and_then(Value::as_str) {
            Some("groupby") => group_by.push(name.clone()),
            Some("aggregate") => {
                let calcs: Vec<String> = spec
                    .get("aggregations")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                aggs.push(Agg {
                    field: name.clone(),
                    calcs,
                });
            }
            _ => {}
        }
    }
    if group_by.is_empty() {
        return frames; // Grafana requires at least one groupby field.
    }
    let mut frames = frames.into_iter();
    let first = match frames.next() {
        Some(f) => f,
        None => return Vec::new(),
    };
    let mut out = vec![group(&first, &group_by, &aggs)];
    out.extend(frames);
    out
}

fn group(frame: &Frame, group_by: &[String], aggs: &[Agg]) -> Frame {
    // Ordered group keys (first-seen) → the source row indices in that group.
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for row in 0..frame.length {
        let key = group_by
            .iter()
            .map(|g| {
                frame
                    .field(g)
                    .map(|f| f.at(row))
                    .unwrap_or(Value::Null)
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\u{1f}");
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push(row);
    }

    // Output: one column per groupby field (a representative value per group), then aggregates.
    let mut fields: Vec<Field> = Vec::new();
    for g in group_by {
        let src = frame.field(g);
        let values: Vec<Value> = order
            .iter()
            .map(|k| {
                let row = groups[k][0];
                src.map(|f| f.at(row)).unwrap_or(Value::Null)
            })
            .collect();
        let ty = src.map(|f| f.ty).unwrap_or(crate::frame::FieldType::Other);
        fields.push(Field::typed(g.clone(), ty, values));
    }
    for agg in aggs {
        let src = match frame.field(&agg.field) {
            Some(f) => f,
            None => continue,
        };
        for calc in &agg.calcs {
            let values: Vec<Value> = order
                .iter()
                .map(|k| {
                    let cells: Vec<Value> = groups[k].iter().map(|&row| src.at(row)).collect();
                    reduce_field(calc, &cells)
                })
                .collect();
            fields.push(Field::new(format!("{} ({})", agg.field, calc), values));
        }
    }
    Frame::new(fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn seeded() -> Frames {
        vec![Frame::new(vec![
            Field::new("host", vec![json!("a"), json!("a"), json!("b")]),
            Field::new("v", vec![json!(10), json!(20), json!(100)]),
        ])]
    }

    #[test]
    fn groups_and_aggregates() {
        let out = apply(
            seeded(),
            &json!({
                "fields": {
                    "host": { "operation": "groupby", "aggregations": [] },
                    "v": { "operation": "aggregate", "aggregations": ["sum", "max"] },
                },
            }),
        );
        assert_eq!(out.len(), 1);
        let frame = &out[0];
        assert_eq!(
            frame.field("host").unwrap().values,
            vec![json!("a"), json!("b")]
        );
        assert_eq!(
            frame.field("v (sum)").unwrap().values,
            vec![json!(30.0), json!(100.0)]
        );
        assert_eq!(
            frame.field("v (max)").unwrap().values,
            vec![json!(20.0), json!(100.0)]
        );
        assert_eq!(frame.length, 2);
    }

    #[test]
    fn no_groupby_passes_through() {
        let out = apply(
            seeded(),
            &json!({ "fields": { "v": { "operation": "aggregate", "aggregations": ["sum"] } } }),
        );
        assert_eq!(out[0].fields.len(), 2);
        assert_eq!(out[0].field("v").unwrap().values.len(), 3);
    }

    #[test]
    fn non_numeric_aggregate_is_null_not_zero() {
        let frames = vec![Frame::new(vec![
            Field::new("host", vec![json!("a"), json!("a")]),
            Field::new("label", vec![json!("x"), json!("y")]),
        ])];
        let out = apply(
            frames,
            &json!({
                "fields": {
                    "host": { "operation": "groupby" },
                    "label": { "operation": "aggregate", "aggregations": ["sum"] },
                },
            }),
        );
        assert_eq!(
            out[0].field("label (sum)").unwrap().values,
            vec![Value::Null]
        );
    }
}
