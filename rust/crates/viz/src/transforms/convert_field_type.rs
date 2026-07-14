//! The `convertFieldType` transformer — Grafana's `ConvertFieldTypeTransformer` (viz
//! grafana-parity scope, tranche 2a). Options verbatim:
//! `{ conversions: [{ targetField, destinationType, dateFormat? }] }`.
//! Conversions, honest-by-cell (a cell that can't convert becomes `Null`, never a guessed value):
//!   - `number`  — numbers pass; numeric strings parse; booleans → 1/0.
//!   - `string`  — any scalar renders to its JSON-literal string (Grafana's `toString`).
//!   - `boolean` — bools pass; numbers → `!= 0`; `"true"`/`"false"` parse.
//!   - `time`    — numbers pass (already canonical epoch-ms); strings parse as RFC 3339 (and
//!     `%Y-%m-%d %H:%M:%S` / `%Y-%m-%d`, read as UTC) → epoch-ms. Grafana's arbitrary
//!     `dateFormat` (dayjs grammar) is NOT ported — an unparsable cell is `Null` (degrade); a
//!     fixture demanding the dayjs grammar names a follow-up, per the tranche bound.
//! An unknown `destinationType` or missing target field leaves the field untouched (carried).

use serde_json::Value;

use crate::frame::{FieldType, Frames};

/// Apply every conversion to every frame that has the target field.
pub fn apply(mut frames: Frames, options: &Value) -> Frames {
    let Some(Value::Array(conversions)) = options.get("conversions") else {
        return frames;
    };
    for c in conversions {
        let target = c.get("targetField").and_then(Value::as_str).unwrap_or("");
        let dest = c
            .get("destinationType")
            .and_then(Value::as_str)
            .unwrap_or("");
        if target.is_empty() {
            continue;
        }
        for frame in &mut frames {
            if let Some(i) = frame.field_index(target) {
                let field = &mut frame.fields[i];
                match dest {
                    "number" => {
                        field.values = field.values.iter().map(to_number).collect();
                        field.ty = FieldType::Number;
                    }
                    "string" => {
                        field.values = field.values.iter().map(to_string_value).collect();
                        field.ty = FieldType::String;
                    }
                    "boolean" => {
                        field.values = field.values.iter().map(to_boolean).collect();
                        field.ty = FieldType::Boolean;
                    }
                    "time" => {
                        field.values = field.values.iter().map(to_time_ms).collect();
                        field.ty = FieldType::Time;
                    }
                    // Unknown destination (enum/other/…): carried untouched.
                    _ => {}
                }
            }
        }
    }
    frames
}

fn to_number(v: &Value) -> Value {
    match v {
        Value::Number(_) => v.clone(),
        Value::Bool(b) => Value::from(if *b { 1 } else { 0 }),
        Value::String(s) => s
            .trim()
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

fn to_string_value(v: &Value) -> Value {
    match v {
        Value::Null => Value::Null,
        Value::String(_) => v.clone(),
        other => Value::from(other.to_string()),
    }
}

fn to_boolean(v: &Value) -> Value {
    match v {
        Value::Bool(_) => v.clone(),
        Value::Number(n) => Value::from(n.as_f64().is_some_and(|f| f != 0.0)),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" => Value::from(true),
            "false" => Value::from(false),
            _ => Value::Null,
        },
        _ => Value::Null,
    }
}

/// A value → canonical epoch-ms. Numbers pass through (already canonical); strings parse as
/// RFC 3339, then the two common bare shapes read as UTC. Anything else → `Null`.
fn to_time_ms(v: &Value) -> Value {
    match v {
        Value::Number(_) => v.clone(),
        Value::String(s) => {
            let s = s.trim();
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                return Value::from(dt.timestamp_millis());
            }
            if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
                return Value::from(ndt.and_utc().timestamp_millis());
            }
            if let Ok(nd) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                return Value::from(
                    nd.and_hms_opt(0, 0, 0)
                        .expect("midnight is valid")
                        .and_utc()
                        .timestamp_millis(),
                );
            }
            Value::Null
        }
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Field, Frame};
    use serde_json::json;

    fn one(name: &str, values: Vec<Value>) -> Frames {
        vec![Frame::new(vec![Field::new(name, values)])]
    }

    fn convert(frames: Frames, field: &str, dest: &str) -> Frames {
        apply(
            frames,
            &json!({ "conversions": [{ "targetField": field, "destinationType": dest }] }),
        )
    }

    #[test]
    fn string_to_number_and_unparsable_is_null() {
        let out = convert(
            one("v", vec![json!("1.5"), json!("x"), json!(2), json!(true)]),
            "v",
            "number",
        );
        assert_eq!(
            out[0].fields[0].values,
            vec![json!(1.5), json!(null), json!(2), json!(1)]
        );
        assert_eq!(out[0].fields[0].ty, FieldType::Number);
    }

    #[test]
    fn to_string_and_to_boolean() {
        let out = convert(one("v", vec![json!(1), json!(true)]), "v", "string");
        assert_eq!(out[0].fields[0].values, vec![json!("1"), json!("true")]);
        let out = convert(
            one("v", vec![json!(0), json!(2), json!("true"), json!("nah")]),
            "v",
            "boolean",
        );
        assert_eq!(
            out[0].fields[0].values,
            vec![json!(false), json!(true), json!(true), json!(null)]
        );
    }

    #[test]
    fn string_to_time_parses_rfc3339_and_bare_utc() {
        let out = convert(
            one(
                "t",
                vec![
                    json!("2026-01-02T00:00:00Z"),
                    json!("2026-01-02 00:00:00"),
                    json!("2026-01-02"),
                    json!(1000),
                    json!("nope"),
                ],
            ),
            "t",
            "time",
        );
        let ms = 1_767_312_000_000i64; // 2026-01-02T00:00:00Z
        assert_eq!(
            out[0].fields[0].values,
            vec![json!(ms), json!(ms), json!(ms), json!(1000), json!(null)]
        );
        assert_eq!(out[0].fields[0].ty, FieldType::Time);
    }

    #[test]
    fn unknown_destination_or_missing_field_is_carried() {
        let out = convert(one("v", vec![json!("1")]), "v", "enum");
        assert_eq!(out[0].fields[0].values, vec![json!("1")]);
        let out = convert(one("v", vec![json!("1")]), "absent", "number");
        assert_eq!(out[0].fields[0].values, vec![json!("1")]);
    }
}
