//! `limit` (viz transformations scope, "Adopt Grafana's transformation model verbatim"). Truncate each
//! frame to the first N rows. Matches Grafana: when `limitField` is undefined the frames pass through
//! unchanged; when present, N is the integer (a string is parsed) and every field's values are sliced
//! to N. Pure + deterministic. One responsibility: the row cap.

use serde_json::Value;

use crate::frame::{Field, Frame, Frames};

/// Parse `limitField` (number | string) to an integer row cap. `None` → pass through unchanged.
fn parse_limit(options: &Value) -> Option<usize> {
    match options.get("limitField") {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => n.as_i64().map(|v| v.max(0) as usize),
        Some(Value::String(s)) => s.trim().parse::<i64>().ok().map(|v| v.max(0) as usize),
        _ => None,
    }
}

fn limit_one(frame: &Frame, n: usize) -> Frame {
    let take = n.min(frame.length);
    let fields: Vec<Field> = frame
        .fields
        .iter()
        .map(|f| {
            let values: Vec<Value> = f.values.iter().take(take).cloned().collect();
            Field::typed(&f.name, f.ty, values)
        })
        .collect();
    let mut out = Frame::new(fields);
    out.ref_id = frame.ref_id.clone();
    out.name = frame.name.clone();
    out.relen()
}

pub fn apply(frames: Frames, options: &Value) -> Frames {
    let n = match parse_limit(options) {
        Some(n) => n,
        None => return frames, // undefined limitField → unchanged
    };
    frames.iter().map(|f| limit_one(f, n)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn frame() -> Frame {
        Frame::new(vec![
            Field::new("a", vec![json!(1), json!(2), json!(3), json!(4)]),
            Field::new("b", vec![json!("w"), json!("x"), json!("y"), json!("z")]),
        ])
    }

    #[test]
    fn truncates_numeric_limit() {
        let out = apply(vec![frame()], &json!({"limitField": 2}));
        let f = &out[0];
        assert_eq!(f.length, 2);
        assert_eq!(f.field("a").unwrap().values, vec![json!(1), json!(2)]);
        assert_eq!(f.field("b").unwrap().values, vec![json!("w"), json!("x")]);
    }

    #[test]
    fn parses_string_limit() {
        let out = apply(vec![frame()], &json!({"limitField": "3"}));
        assert_eq!(out[0].length, 3);
    }

    #[test]
    fn undefined_passes_through() {
        let out = apply(vec![frame()], &json!({}));
        assert_eq!(out[0].length, 4);
    }

    #[test]
    fn limit_beyond_length_is_noop_and_keeps_real_values() {
        let out = apply(vec![frame()], &json!({"limitField": 99}));
        // No padding with fabricated rows — the real 4 rows stay as-is.
        assert_eq!(out[0].length, 4);
        assert_eq!(
            out[0].field("a").unwrap().values,
            vec![json!(1), json!(2), json!(3), json!(4)]
        );
    }
}
