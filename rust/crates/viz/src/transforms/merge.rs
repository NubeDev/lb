//! `merge` (viz transformations scope, "Adopt Grafana's transformation model verbatim"). Stack the
//! rows of all input frames into ONE frame. The output field set is the UNION of all field names
//! (first-seen order); each input row contributes its values, Null for fields it lacks (a ragged
//! result is honest, not coerced). Single frame → pass through. Pure + deterministic. One
//! responsibility: the row stack.

use serde_json::Value;

use crate::frame::{Field, FieldType, Frame, Frames};

pub fn apply(frames: Frames, _options: &Value) -> Frames {
    if frames.len() <= 1 {
        return frames;
    }

    // Union of field names in first-seen order; remember a representative type per name.
    let mut order: Vec<String> = Vec::new();
    let mut ty: std::collections::HashMap<String, FieldType> = std::collections::HashMap::new();
    for f in &frames {
        for field in &f.fields {
            if !order.iter().any(|n| n == &field.name) {
                order.push(field.name.clone());
                ty.insert(field.name.clone(), field.ty);
            }
        }
    }

    // Build each output column by appending every frame's rows (Null where the frame lacks the field).
    let mut columns: Vec<Vec<Value>> = order.iter().map(|_| Vec::new()).collect();
    for f in &frames {
        for row in 0..f.length {
            for (ci, name) in order.iter().enumerate() {
                let v = f.field(name).map(|fld| fld.at(row)).unwrap_or(Value::Null);
                columns[ci].push(v);
            }
        }
    }

    let fields: Vec<Field> = order
        .into_iter()
        .zip(columns)
        .map(|(name, values)| {
            let t = *ty.get(&name).unwrap_or(&FieldType::Other);
            Field::typed(name, t, values)
        })
        .collect();

    let mut out = Frame::new(fields);
    out.ref_id = frames[0].ref_id.clone();
    vec![out.relen()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stacks_rows_with_union_of_fields() {
        let a = Frame::new(vec![
            Field::new("x", vec![json!(1), json!(2)]),
            Field::new("y", vec![json!(10), json!(20)]),
        ]);
        let b = Frame::new(vec![
            Field::new("x", vec![json!(3)]),
            Field::new("z", vec![json!(300)]),
        ]);
        let out = apply(vec![a, b], &json!({}));
        assert_eq!(out.len(), 1);
        let f = &out[0];
        assert_eq!(f.length, 3);
        assert_eq!(
            f.field("x").unwrap().values,
            vec![json!(1), json!(2), json!(3)]
        );
        // y missing in b's row → Null (not 0); z missing in a's rows → Null.
        assert_eq!(
            f.field("y").unwrap().values,
            vec![json!(10), json!(20), Value::Null]
        );
        assert_eq!(
            f.field("z").unwrap().values,
            vec![Value::Null, Value::Null, json!(300)]
        );
    }

    #[test]
    fn single_frame_passes_through() {
        let a = Frame::new(vec![Field::new("x", vec![json!(1)])]);
        let out = apply(vec![a.clone()], &json!({}));
        assert_eq!(out, vec![a]);
    }

    #[test]
    fn missing_fields_are_null_never_fabricated() {
        let a = Frame::new(vec![Field::new("a", vec![json!(5)])]);
        let b = Frame::new(vec![Field::new("b", vec![json!(7)])]);
        let out = apply(vec![a, b], &json!({}));
        let f = &out[0];
        assert_eq!(f.field("a").unwrap().values, vec![json!(5), Value::Null]);
        assert_eq!(f.field("b").unwrap().values, vec![Value::Null, json!(7)]);
    }
}
