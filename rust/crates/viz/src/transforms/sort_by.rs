//! `sortBy` (viz transformations scope, "Adopt Grafana's transformation model verbatim"). Stable-sort
//! each frame's ROWS by the first sort entry's field (numeric compare when both cells numeric, else
//! string compare; nulls last); `desc` reverses. Every field's values are reordered by the same row
//! permutation. Pure + deterministic. One responsibility: the row sort.

use std::cmp::Ordering;

use serde_json::Value;

use crate::frame::{Field, Frame, Frames};

/// Compare two cells: numeric when both parse as f64, else string-ish; nulls sort last.
fn cmp_cells(a: &Value, b: &Value) -> Ordering {
    match (a.is_null(), b.is_null()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater, // nulls last
        (false, true) => Ordering::Less,
        (false, false) => match (a.as_f64(), b.as_f64()) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
            _ => cell_str(a).cmp(&cell_str(b)),
        },
    }
}

fn cell_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn sort_one(frame: &Frame, field_name: &str, desc: bool) -> Frame {
    let key = match frame.field(field_name) {
        Some(f) => f,
        None => return frame.clone(),
    };
    // Stable sort of row indices by the key field; `sort_by` is stable in Rust.
    let mut order: Vec<usize> = (0..frame.length).collect();
    order.sort_by(|&i, &j| {
        let c = cmp_cells(&key.at(i), &key.at(j));
        if desc {
            c.reverse()
        } else {
            c
        }
    });

    let fields: Vec<Field> = frame
        .fields
        .iter()
        .map(|f| {
            let values: Vec<Value> = order.iter().map(|&i| f.at(i)).collect();
            Field::typed(&f.name, f.ty, values)
        })
        .collect();
    let mut out = Frame::new(fields);
    out.ref_id = frame.ref_id.clone();
    out.name = frame.name.clone();
    out.relen()
}

pub fn apply(frames: Frames, options: &Value) -> Frames {
    let first = options
        .get("sort")
        .and_then(Value::as_array)
        .and_then(|a| a.first());
    let (field, desc) = match first {
        Some(entry) => (
            entry
                .get("field")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            entry.get("desc").and_then(Value::as_bool).unwrap_or(false),
        ),
        None => return frames, // no sort spec → pass through
    };
    if field.is_empty() {
        return frames;
    }
    frames.iter().map(|f| sort_one(f, &field, desc)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn frame() -> Frame {
        Frame::new(vec![
            Field::new("v", vec![json!(3), json!(1), json!(2)]),
            Field::new("tag", vec![json!("c"), json!("a"), json!("b")]),
        ])
    }

    #[test]
    fn sorts_ascending_and_reorders_all_fields() {
        let out = apply(vec![frame()], &json!({"sort":[{"field":"v"}]}));
        let f = &out[0];
        assert_eq!(
            f.field("v").unwrap().values,
            vec![json!(1), json!(2), json!(3)]
        );
        assert_eq!(
            f.field("tag").unwrap().values,
            vec![json!("a"), json!("b"), json!("c")]
        );
    }

    #[test]
    fn sorts_descending() {
        let out = apply(vec![frame()], &json!({"sort":[{"field":"v","desc":true}]}));
        assert_eq!(
            out[0].field("v").unwrap().values,
            vec![json!(3), json!(2), json!(1)]
        );
    }

    #[test]
    fn nulls_sort_last_no_fabricated_value() {
        let f = Frame::new(vec![Field::new("v", vec![json!(2), Value::Null, json!(1)])]);
        let out = apply(vec![f], &json!({"sort":[{"field":"v"}]}));
        // 1, 2, then null at the end — the null is preserved, not coerced to 0.
        assert_eq!(
            out[0].field("v").unwrap().values,
            vec![json!(1), json!(2), Value::Null]
        );
    }

    #[test]
    fn missing_field_passes_through() {
        let out = apply(vec![frame()], &json!({"sort":[{"field":"nope"}]}));
        assert_eq!(
            out[0].field("v").unwrap().values,
            vec![json!(3), json!(1), json!(2)]
        );
    }

    #[test]
    fn string_sort_when_non_numeric() {
        let f = Frame::new(vec![Field::new(
            "s",
            vec![json!("banana"), json!("apple"), json!("cherry")],
        )]);
        let out = apply(vec![f], &json!({"sort":[{"field":"s"}]}));
        assert_eq!(
            out[0].field("s").unwrap().values,
            vec![json!("apple"), json!("banana"), json!("cherry")]
        );
    }
}
