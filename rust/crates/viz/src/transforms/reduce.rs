//! The `reduce` transformer — Grafana's `ReduceTransformer` (viz transformations scope). Collapses
//! columns to summary values via the shared `reduce_field`. Two modes, Grafana-verbatim:
//! `seriesToRows` (default) → one frame, a `Field` column naming each input field + one column per
//! reducer holding that reducer over each field; `reduceFields` → each frame collapses to a single
//! row, each field reduced by the first reducer. One responsibility: the reduce step. Honest: an
//! empty/non-numeric column reduces to `Null`, never a fabricated 0 (the no-mock rule).

use serde_json::Value;

use crate::frame::{Field, FieldType, Frame, Frames};
use crate::reducer::reduce_field;

/// Apply `reduce`. `options.reducers` is the list of `ReducerID`s; `options.mode` is
/// `reduceFields`|`seriesToRows` (default `seriesToRows`). With no reducers we pass frames through
/// unchanged (nothing to compute — honest no-op).
pub fn apply(frames: Frames, options: &Value) -> Frames {
    let reducers: Vec<String> = options
        .get("reducers")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    if reducers.is_empty() {
        return frames;
    }
    let mode = options
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("seriesToRows");
    let include_time = options
        .get("includeTimeField")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    match mode {
        "reduceFields" => frames
            .into_iter()
            .map(|f| reduce_fields(f, &reducers, include_time))
            .collect(),
        _ => vec![series_to_rows(frames, &reducers, include_time)],
    }
}

/// `reduceFields`: collapse each frame to ONE row, each field reduced by the FIRST reducer. The time
/// field is dropped unless `includeTimeField` (Grafana: a reduced time has no meaning by default).
fn reduce_fields(frame: Frame, reducers: &[String], include_time: bool) -> Frame {
    let calc = &reducers[0];
    let fields: Vec<Field> = frame
        .fields
        .iter()
        .filter(|f| include_time || f.ty != FieldType::Time)
        .map(|f| {
            let reduced = reduce_field(calc, &f.values);
            Field::new(f.name.clone(), vec![reduced])
        })
        .collect();
    let mut out = Frame::new(fields);
    out.ref_id = frame.ref_id;
    out.name = frame.name;
    out
}

/// `seriesToRows` (default): ONE output frame. A `Field` column lists each input field name; each
/// reducer becomes a column (`Max`, `Mean`, …) holding that reducer over the matching field. Across
/// frames the field rows are concatenated in encounter order.
fn series_to_rows(frames: Frames, reducers: &[String], include_time: bool) -> Frame {
    let mut names: Vec<Value> = Vec::new();
    let mut reduced: Vec<Vec<Value>> = vec![Vec::new(); reducers.len()];
    for frame in &frames {
        for f in &frame.fields {
            if !include_time && f.ty == FieldType::Time {
                continue;
            }
            names.push(Value::from(f.name.clone()));
            for (i, calc) in reducers.iter().enumerate() {
                reduced[i].push(reduce_field(calc, &f.values));
            }
        }
    }
    let mut fields = vec![Field::typed("Field", FieldType::String, names)];
    for (i, calc) in reducers.iter().enumerate() {
        fields.push(Field::new(
            reducer_label(calc),
            std::mem::take(&mut reduced[i]),
        ));
    }
    Frame::new(fields)
}

/// A reducer's display label — the Grafana column title (`max` → `Max`, `mean` → `Mean`). Falls back
/// to the raw id capitalized so an uncommon calc still gets an honest, stable header.
fn reducer_label(calc: &str) -> String {
    match calc {
        "sum" => "Sum",
        "mean" | "avg" => "Mean",
        "min" => "Min",
        "max" => "Max",
        "range" => "Range",
        "count" => "Count",
        "first" => "First",
        "firstNotNull" => "First (not null)",
        "last" => "Last",
        "lastNotNull" => "Last (not null)",
        other => return capitalize(other),
    }
    .to_string()
}

/// Capitalize the first char (ASCII) — a stable fallback header for an uncommon reducer id.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn seeded() -> Frames {
        vec![Frame::new(vec![
            Field::typed("time", FieldType::Time, vec![json!(1), json!(2), json!(3)]),
            Field::new("temp", vec![json!(10), json!(20), json!(30)]),
        ])]
    }

    #[test]
    fn series_to_rows_default_one_row_per_field() {
        let out = apply(seeded(), &json!({ "reducers": ["max", "mean"] }));
        assert_eq!(out.len(), 1);
        let frame = &out[0];
        // time excluded by default → only the temp field row.
        assert_eq!(frame.field("Field").unwrap().values, vec![json!("temp")]);
        assert_eq!(frame.field("Max").unwrap().values, vec![json!(30.0)]);
        assert_eq!(frame.field("Mean").unwrap().values, vec![json!(20.0)]);
    }

    #[test]
    fn reduce_fields_collapses_to_single_row() {
        let out = apply(
            seeded(),
            &json!({ "mode": "reduceFields", "reducers": ["sum"], "includeTimeField": true }),
        );
        assert_eq!(out.len(), 1);
        let frame = &out[0];
        assert_eq!(frame.length, 1);
        assert_eq!(frame.field("temp").unwrap().values, vec![json!(60.0)]);
        assert_eq!(frame.field("time").unwrap().values, vec![json!(6.0)]);
    }

    #[test]
    fn empty_reducers_passes_through() {
        let out = apply(seeded(), &json!({ "reducers": [] }));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].fields.len(), 2);
    }

    #[test]
    fn non_numeric_column_reduces_to_null_not_zero() {
        let frames = vec![Frame::new(vec![Field::new(
            "label",
            vec![json!("a"), json!("b")],
        )])];
        let out = apply(frames, &json!({ "reducers": ["sum"] }));
        let frame = &out[0];
        assert_eq!(frame.field("Field").unwrap().values, vec![json!("label")]);
        assert_eq!(frame.field("Sum").unwrap().values, vec![Value::Null]);
    }
}
