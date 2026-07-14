//! The `extractFields` transformer — Grafana's `ExtractFieldsTransformer` (viz grafana-parity
//! scope, tranche 2a). Options verbatim: `{ source, format?, replace?, keepTime? }`. Each cell of
//! the `source` field is parsed into key/value pairs and the keys become new fields:
//!   - `format: "json"` — the cell is a JSON object (a JSON string is parsed; an object value is
//!     used as-is).
//!   - `format: "kv"` — `key=value` pairs separated by `,`/space (Grafana's key-values grammar).
//!   - `format: "auto"`/absent — JSON when the cell parses as an object, else kv.
//! New columns appear in first-seen key order (alphabetical within one cell — `serde_json::Map`
//! sorts keys; column ORDER is cosmetic, addressing is by name); a row without a key reads `Null`
//! (ragged-honest).
//! `replace: true` keeps ONLY the extracted fields (+ time fields when `keepTime: true`). A cell
//! that parses to nothing contributes nothing (never an error). Grafana's `delimiter`/`regexp`
//! formats are not ported (tranche bound — carried as an unknown format, source untouched).

use serde_json::{Map, Value};

use crate::frame::{Field, FieldType, Frame, Frames};

/// Apply `extractFields` to every frame carrying the source field.
pub fn apply(frames: Frames, options: &Value) -> Frames {
    let source = options.get("source").and_then(Value::as_str).unwrap_or("");
    let format = options
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("auto");
    let replace = options
        .get("replace")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let keep_time = options
        .get("keepTime")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if source.is_empty() || !matches!(format, "json" | "kv" | "auto") {
        return frames;
    }
    frames
        .into_iter()
        .map(|f| extract(f, source, format, replace, keep_time))
        .collect()
}

fn extract(frame: Frame, source: &str, format: &str, replace: bool, keep_time: bool) -> Frame {
    let Some(src) = frame.field(source) else {
        return frame;
    };

    // Parse every cell; collect the union of keys in first-seen order.
    let parsed: Vec<Map<String, Value>> =
        src.values.iter().map(|v| parse_cell(v, format)).collect();
    let mut order: Vec<String> = Vec::new();
    for row in &parsed {
        for k in row.keys() {
            if !order.iter().any(|o| o == k) {
                order.push(k.clone());
            }
        }
    }

    let extracted: Vec<Field> = order
        .iter()
        .map(|key| {
            let values: Vec<Value> = parsed
                .iter()
                .map(|row| row.get(key).cloned().unwrap_or(Value::Null))
                .collect();
            Field::new(key.clone(), values)
        })
        .collect();

    let mut fields: Vec<Field> = if replace {
        frame
            .fields
            .into_iter()
            .filter(|f| keep_time && f.ty == FieldType::Time)
            .collect()
    } else {
        frame.fields
    };
    fields.extend(extracted);

    let mut out = Frame::new(fields).relen();
    out.ref_id = frame.ref_id;
    out.name = frame.name;
    out
}

/// One cell → key/value pairs. Unparsable → empty (that row reads null everywhere).
fn parse_cell(v: &Value, format: &str) -> Map<String, Value> {
    match v {
        Value::Object(m) if format != "kv" => m.clone(),
        Value::String(s) => {
            if format != "kv" {
                if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(s) {
                    return m;
                }
                if format == "json" {
                    return Map::new();
                }
            }
            parse_kv(s)
        }
        _ => Map::new(),
    }
}

/// The kv grammar: `key=value` pairs split on `,` or whitespace.
fn parse_kv(s: &str) -> Map<String, Value> {
    let mut out = Map::new();
    for pair in s.split([',', ' ']).filter(|p| !p.is_empty()) {
        if let Some((k, v)) = pair.split_once('=') {
            out.insert(k.trim().to_string(), Value::from(v.trim()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn seeded() -> Frames {
        let mut time = Field::new("ts", vec![json!(1), json!(2)]);
        time.ty = FieldType::Time;
        vec![Frame::new(vec![
            time,
            Field::new(
                "payload",
                vec![
                    json!(r#"{"temp": 21, "mode": "cool"}"#),
                    json!({"temp": 22}),
                ],
            ),
        ])]
    }

    #[test]
    fn json_cells_become_fields_ragged_reads_null() {
        let out = apply(seeded(), &json!({ "source": "payload", "format": "json" }));
        let f = &out[0];
        assert_eq!(f.field("temp").unwrap().values, vec![json!(21), json!(22)]);
        assert_eq!(
            f.field("mode").unwrap().values,
            vec![json!("cool"), json!(null)]
        );
        // Source kept when replace is off.
        assert!(f.field("payload").is_some());
    }

    #[test]
    fn replace_keeps_only_extracted_plus_time_when_asked() {
        let out = apply(
            seeded(),
            &json!({ "source": "payload", "replace": true, "keepTime": true }),
        );
        let names: Vec<&str> = out[0].fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["ts", "mode", "temp"]); // keys alphabetical (serde_json Map)
        let out = apply(seeded(), &json!({ "source": "payload", "replace": true }));
        let names: Vec<&str> = out[0].fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["mode", "temp"]);
    }

    #[test]
    fn kv_grammar_parses_pairs() {
        let frames = vec![Frame::new(vec![Field::new(
            "msg",
            vec![json!("a=1, b=x"), json!("a=2")],
        )])];
        let out = apply(frames, &json!({ "source": "msg", "format": "kv" }));
        assert_eq!(
            out[0].field("a").unwrap().values,
            vec![json!("1"), json!("2")]
        );
        assert_eq!(
            out[0].field("b").unwrap().values,
            vec![json!("x"), json!(null)]
        );
    }

    #[test]
    fn missing_source_or_unknown_format_is_carried() {
        let out = apply(seeded(), &json!({ "source": "absent" }));
        assert_eq!(out[0].fields.len(), 2);
        let out = apply(
            seeded(),
            &json!({ "source": "payload", "format": "regexp" }),
        );
        assert_eq!(out[0].fields.len(), 2);
    }
}
