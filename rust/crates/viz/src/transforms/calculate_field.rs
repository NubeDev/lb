//! `calculateField` (viz transformations scope, "Adopt Grafana's transformation model verbatim").
//! Adds a derived numeric field per frame via a BOUNDED set of modes — binary (field/fixed op
//! field/fixed), reduceRow (reduce the row's numeric cells), index (0-based row index), unary
//! (abs/neg/floor/ceil). NO arbitrary eval — only these modes. Pure + deterministic. One
//! responsibility: the derived-field calc.

use serde_json::Value;

use crate::frame::{Field, FieldType, Frame, Frames};
use crate::reducer::reduce_field;

/// One side of a binary op — a field reference (by name) or a fixed numeric literal.
fn side_value(frame: &Frame, side: &Value, row: usize) -> Option<f64> {
    if let Some(name) = side
        .get("field")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return frame.field(name).and_then(|f| f.num_at(row));
    }
    if let Some(fixed) = side.get("fixed") {
        return match fixed {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        };
    }
    None
}

/// Label for a binary side (for the default alias).
fn side_label(side: &Value) -> String {
    side.get("field")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            side.get("fixed").map(|f| match f {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
        })
        .unwrap_or_default()
}

/// A finite JSON number, or Null (honest — NaN/inf/None never become 0).
fn num(opt: Option<f64>) -> Value {
    match opt {
        Some(f) if f.is_finite() => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

fn binary_field(frame: &Frame, binary: &Value, alias: Option<&str>) -> Field {
    let left = binary.get("left").cloned().unwrap_or(Value::Null);
    let right = binary.get("right").cloned().unwrap_or(Value::Null);
    let op = binary
        .get("operator")
        .and_then(Value::as_str)
        .unwrap_or("+");
    let values: Vec<Value> = (0..frame.length)
        .map(|row| {
            let l = side_value(frame, &left, row);
            let r = side_value(frame, &right, row);
            match (l, r) {
                (Some(a), Some(b)) => num(apply_op(op, a, b)),
                _ => Value::Null,
            }
        })
        .collect();
    let name = alias
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{} {} {}", side_label(&left), op, side_label(&right)));
    Field::typed(name, FieldType::Number, values)
}

fn apply_op(op: &str, a: f64, b: f64) -> Option<f64> {
    match op {
        "+" => Some(a + b),
        "-" => Some(a - b),
        "*" => Some(a * b),
        "/" => {
            if b == 0.0 {
                None
            } else {
                Some(a / b)
            }
        }
        _ => None,
    }
}

fn reduce_row_field(frame: &Frame, reduce: &Value, alias: Option<&str>) -> Field {
    let reducer = reduce
        .get("reducer")
        .and_then(Value::as_str)
        .unwrap_or("sum");
    let include: Option<Vec<String>> = reduce.get("include").and_then(Value::as_array).map(|a| {
        a.iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    });
    // Which fields contribute per row: the include list, else all numeric fields.
    let src: Vec<&Field> = frame
        .fields
        .iter()
        .filter(|f| match &include {
            Some(names) => names.iter().any(|n| n == &f.name),
            None => f.ty == FieldType::Number,
        })
        .collect();
    let values: Vec<Value> = (0..frame.length)
        .map(|row| {
            let cells: Vec<Value> = src.iter().map(|f| f.at(row)).collect();
            reduce_field(reducer, &cells)
        })
        .collect();
    let name = alias
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| reducer.to_string());
    Field::typed(name, FieldType::Number, values)
}

fn index_field(frame: &Frame, alias: Option<&str>) -> Field {
    let values: Vec<Value> = (0..frame.length).map(|i| Value::from(i)).collect();
    let name = alias
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Row".into());
    Field::typed(name, FieldType::Number, values)
}

fn unary_field(frame: &Frame, unary: &Value, alias: Option<&str>) -> Field {
    let op = unary
        .get("operator")
        .and_then(Value::as_str)
        .unwrap_or("abs");
    let field_name = unary.get("fieldName").and_then(Value::as_str).unwrap_or("");
    let values: Vec<Value> = (0..frame.length)
        .map(|row| {
            let v = frame.field(field_name).and_then(|f| f.num_at(row));
            num(v.map(|x| match op {
                "abs" => x.abs(),
                "neg" => -x,
                "floor" => x.floor(),
                "ceil" => x.ceil(),
                _ => x,
            }))
        })
        .collect();
    let name = alias
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{op}({field_name})"));
    Field::typed(name, FieldType::Number, values)
}

fn calc_one(frame: &Frame, options: &Value) -> Frame {
    let mode = options
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("binary");
    let alias = options.get("alias").and_then(Value::as_str);
    let replace = options
        .get("replaceFields")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let new_field = match mode {
        "binary" => binary_field(
            frame,
            &options.get("binary").cloned().unwrap_or(Value::Null),
            alias,
        ),
        "reduceRow" => reduce_row_field(
            frame,
            &options.get("reduce").cloned().unwrap_or(Value::Null),
            alias,
        ),
        "index" => index_field(frame, alias),
        "unary" => unary_field(
            frame,
            &options.get("unary").cloned().unwrap_or(Value::Null),
            alias,
        ),
        _ => return frame.clone(),
    };

    let mut fields: Vec<Field> = if replace {
        // Keep any time field(s), then the new field.
        frame
            .fields
            .iter()
            .filter(|f| f.ty == FieldType::Time)
            .cloned()
            .collect()
    } else {
        frame.fields.clone()
    };
    fields.push(new_field);
    let mut out = Frame::new(fields);
    out.ref_id = frame.ref_id.clone();
    out.name = frame.name.clone();
    out.relen()
}

pub fn apply(frames: Frames, options: &Value) -> Frames {
    frames.iter().map(|f| calc_one(f, options)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn frame() -> Frame {
        Frame::new(vec![
            Field::typed("time", FieldType::Time, vec![json!(1), json!(2)]),
            Field::new("a", vec![json!(10), json!(20)]),
            Field::new("b", vec![json!(4), json!(0)]),
        ])
    }

    #[test]
    fn binary_add_and_default_alias() {
        let opts = json!({"mode":"binary","binary":{"left":{"field":"a"},"operator":"+","right":{"field":"b"}}});
        let out = apply(vec![frame()], &opts);
        let f = out[0].field("a + b").unwrap();
        assert_eq!(f.values, vec![json!(14.0), json!(20.0)]);
    }

    #[test]
    fn binary_divide_by_zero_is_null_not_zero() {
        let opts = json!({"mode":"binary","binary":{"left":{"field":"a"},"operator":"/","right":{"field":"b"}}});
        let out = apply(vec![frame()], &opts);
        let f = out[0].field("a / b").unwrap();
        assert_eq!(f.at(0), json!(2.5));
        assert_eq!(f.at(1), Value::Null); // 20 / 0 → honest Null
    }

    #[test]
    fn reduce_row_sum_and_non_numeric_is_null() {
        let opts = json!({"mode":"reduceRow","reduce":{"reducer":"sum","include":["a","b"]}});
        let out = apply(vec![frame()], &opts);
        let f = out[0].field("sum").unwrap();
        assert_eq!(f.values, vec![json!(14.0), json!(20.0)]);

        // A frame of only non-numeric cells reduces to Null, never 0.
        let nf = Frame::new(vec![Field::new("s", vec![json!("x"), json!("y")])]);
        let out2 = apply(
            vec![nf],
            &json!({"mode":"reduceRow","reduce":{"reducer":"sum"}}),
        );
        assert_eq!(
            out2[0].field("sum").unwrap().values,
            vec![Value::Null, Value::Null]
        );
    }

    #[test]
    fn index_mode_numbers_rows() {
        let out = apply(vec![frame()], &json!({"mode":"index"}));
        assert_eq!(
            out[0].field("Row").unwrap().values,
            vec![json!(0), json!(1)]
        );
    }

    #[test]
    fn unary_neg_and_replace_keeps_time() {
        let opts =
            json!({"mode":"unary","unary":{"operator":"neg","fieldName":"a"},"replaceFields":true});
        let out = apply(vec![frame()], &opts);
        let f = &out[0];
        assert!(f.field("time").is_some());
        assert!(f.field("a").is_none()); // replaced
        assert_eq!(
            f.field("neg(a)").unwrap().values,
            vec![json!(-10.0), json!(-20.0)]
        );
    }
}
