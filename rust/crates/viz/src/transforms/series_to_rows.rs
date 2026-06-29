//! `seriesToRows` (viz transformations scope, "Adopt Grafana's transformation model verbatim").
//! Combine multiple single-value time series into ONE frame with columns `Time`, `Metric` (the source
//! frame's name/refId), `Value` (the source frame's first numeric non-time value). Output is one frame
//! sorted by time. Empty in → empty frame out. Pure + deterministic. One responsibility: the
//! series→rows reshape.

use std::cmp::Ordering;

use serde_json::Value;

use crate::frame::{Field, FieldType, Frame, Frames};

/// The frame's time field (typed `Time`, else a field literally named "time"/"Time").
fn time_field(frame: &Frame) -> Option<&Field> {
    frame
        .fields
        .iter()
        .find(|f| f.ty == FieldType::Time)
        .or_else(|| {
            frame
                .fields
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case("time"))
        })
}

/// The frame's first numeric, non-time value field.
fn value_field<'a>(frame: &'a Frame, time: Option<&Field>) -> Option<&'a Field> {
    let time_name = time.map(|t| t.name.as_str());
    frame
        .fields
        .iter()
        .find(|f| f.ty == FieldType::Number && Some(f.name.as_str()) != time_name)
}

pub fn apply(frames: Frames, _options: &Value) -> Frames {
    if frames.is_empty() {
        return frames;
    }

    let mut times: Vec<Value> = Vec::new();
    let mut metrics: Vec<Value> = Vec::new();
    let mut values: Vec<Value> = Vec::new();

    for f in &frames {
        let tf = time_field(f);
        let vf = value_field(f, tf);
        let metric = if !f.name.is_empty() {
            f.name.clone()
        } else {
            f.ref_id.clone()
        };
        for row in 0..f.length {
            times.push(tf.map(|t| t.at(row)).unwrap_or(Value::Null));
            metrics.push(Value::from(metric.clone()));
            values.push(vf.map(|v| v.at(row)).unwrap_or(Value::Null));
        }
    }

    // Sort the three columns together by time (numeric epoch-ms; nulls last).
    let mut order: Vec<usize> = (0..times.len()).collect();
    order.sort_by(|&i, &j| cmp_time(&times[i], &times[j]));

    let time_out: Vec<Value> = order.iter().map(|&i| times[i].clone()).collect();
    let metric_out: Vec<Value> = order.iter().map(|&i| metrics[i].clone()).collect();
    let value_out: Vec<Value> = order.iter().map(|&i| values[i].clone()).collect();

    let frame = Frame::new(vec![
        Field::typed("Time", FieldType::Time, time_out),
        Field::typed("Metric", FieldType::String, metric_out),
        Field::typed("Value", FieldType::Number, value_out),
    ]);
    vec![frame.relen()]
}

fn cmp_time(a: &Value, b: &Value) -> Ordering {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn series(name: &str, t: i64, v: Value) -> Frame {
        let mut f = Frame::new(vec![
            Field::typed("time", FieldType::Time, vec![json!(t)]),
            Field::new("value", vec![v]),
        ]);
        f.name = name.into();
        f
    }

    #[test]
    fn combines_and_sorts_by_time() {
        let a = series("cpu", 200, json!(0.5));
        let b = series("mem", 100, json!(42));
        let out = apply(vec![a, b], &json!({}));
        assert_eq!(out.len(), 1);
        let f = &out[0];
        assert_eq!(f.length, 2);
        // sorted by time: 100 (mem) then 200 (cpu)
        assert_eq!(
            f.field("Time").unwrap().values,
            vec![json!(100), json!(200)]
        );
        assert_eq!(
            f.field("Metric").unwrap().values,
            vec![json!("mem"), json!("cpu")]
        );
        assert_eq!(
            f.field("Value").unwrap().values,
            vec![json!(42), json!(0.5)]
        );
    }

    #[test]
    fn empty_in_empty_out() {
        let out = apply(vec![], &json!({}));
        assert!(out.is_empty());
    }

    #[test]
    fn non_numeric_value_yields_null_not_zero() {
        // A frame with no numeric value field → Value is honest Null.
        let mut f = Frame::new(vec![
            Field::typed("time", FieldType::Time, vec![json!(1)]),
            Field::new("label", vec![json!("text")]),
        ]);
        f.name = "s".into();
        let out = apply(vec![f], &json!({}));
        assert_eq!(out[0].field("Value").unwrap().at(0), Value::Null);
        assert_eq!(out[0].field("Metric").unwrap().at(0), json!("s"));
    }
}
