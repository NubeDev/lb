//! `csv` / `yaml` / `base64` — the line-oriented and whole-value text converters (data-nodes Parse).
//! Pure; malformed input FAILS the node (json-node parity). Cells stay strings on the way in — the
//! decoders never infer number types; the `range`/predicate layer coerces later.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{Map, Value};

use super::cell_string;

/// csv ↔ array-of-objects. `parse` decodes a CSV-text JSON string into an array (of objects when
/// `config.header` is true — the default — else an array of string-cell rows). `stringify` renders a
/// JSON array back to a CSV JSON string (a header row = the first-seen union of keys for objects,
/// bare cell rows for arrays). `config.separator`'s first byte is the delimiter. Any parse error,
/// non-array `stringify` payload, or unknown mode is `Err`.
pub fn csv(config: &Value, payload: &Value, mode: &str) -> Result<Value, String> {
    let sep = config
        .get("separator")
        .and_then(Value::as_str)
        .and_then(|s| s.bytes().next())
        .unwrap_or(b',');
    let header = config
        .get("header")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    match mode {
        "parse" => {
            let text = payload
                .as_str()
                .ok_or("csv parse: payload must be a JSON string of CSV text")?;
            let mut rdr = csv::ReaderBuilder::new()
                .delimiter(sep)
                .has_headers(header)
                .flexible(false)
                .from_reader(text.as_bytes());
            let mut out: Vec<Value> = Vec::new();
            if header {
                let keys: Vec<String> = rdr
                    .headers()
                    .map_err(|e| format!("csv parse: bad header — {e}"))?
                    .iter()
                    .map(str::to_owned)
                    .collect();
                for rec in rdr.records() {
                    let rec = rec.map_err(|e| format!("csv parse: {e}"))?;
                    let mut obj = Map::new();
                    for (k, cell) in keys.iter().zip(rec.iter()) {
                        obj.insert(k.clone(), Value::String(cell.to_owned()));
                    }
                    out.push(Value::Object(obj));
                }
            } else {
                for rec in rdr.records() {
                    let rec = rec.map_err(|e| format!("csv parse: {e}"))?;
                    out.push(Value::Array(
                        rec.iter().map(|c| Value::String(c.to_owned())).collect(),
                    ));
                }
            }
            Ok(Value::Array(out))
        }
        "stringify" => {
            let rows = payload
                .as_array()
                .ok_or("csv stringify: payload must be an array")?;
            let mut wtr = csv::WriterBuilder::new().delimiter(sep).from_writer(vec![]);
            let objects = rows.iter().all(Value::is_object);
            if objects && !rows.is_empty() {
                let mut keys: Vec<String> = Vec::new();
                for row in rows {
                    for k in row.as_object().unwrap().keys() {
                        if !keys.contains(k) {
                            keys.push(k.clone());
                        }
                    }
                }
                wtr.write_record(&keys).map_err(|e| e.to_string())?;
                for row in rows {
                    let obj = row.as_object().unwrap();
                    let cells: Vec<String> = keys.iter().map(|k| cell_string(obj.get(k))).collect();
                    wtr.write_record(&cells).map_err(|e| e.to_string())?;
                }
            } else {
                for row in rows {
                    let cells: Vec<String> = row
                        .as_array()
                        .ok_or("csv stringify: expected an array-of-arrays row")?
                        .iter()
                        .map(|c| cell_string(Some(c)))
                        .collect();
                    wtr.write_record(&cells).map_err(|e| e.to_string())?;
                }
            }
            let bytes = wtr.into_inner().map_err(|e| e.to_string())?;
            let s = String::from_utf8(bytes).map_err(|e| e.to_string())?;
            Ok(Value::String(s))
        }
        other => Err(format!("csv: unknown mode {other:?}")),
    }
}

/// yaml ↔ structured value. `parse` decodes a YAML JSON string into the value; `stringify` renders
/// any payload to a YAML JSON string. Malformed YAML or an unknown mode is `Err`.
pub fn yaml(_config: &Value, payload: &Value, mode: &str) -> Result<Value, String> {
    match mode {
        "parse" => {
            let text = payload
                .as_str()
                .ok_or("yaml parse: payload must be a JSON string of YAML")?;
            serde_yaml::from_str::<Value>(text).map_err(|e| format!("yaml parse: {e}"))
        }
        "stringify" => {
            let s = serde_yaml::to_string(payload).map_err(|e| format!("yaml stringify: {e}"))?;
            Ok(Value::String(s))
        }
        other => Err(format!("yaml: unknown mode {other:?}")),
    }
}

/// base64. `encode` turns a payload string (or, for non-strings, its compact JSON) into a base64 JSON
/// string; `decode` turns a base64 JSON string back into the decoded UTF-8 text. Invalid base64,
/// non-UTF-8 decoded bytes, or an unknown mode is `Err`.
pub fn base64(_config: &Value, payload: &Value, mode: &str) -> Result<Value, String> {
    match mode {
        "encode" => {
            let bytes = match payload {
                Value::String(s) => s.clone().into_bytes(),
                other => other.to_string().into_bytes(),
            };
            Ok(Value::String(STANDARD.encode(bytes)))
        }
        "decode" => {
            let s = payload
                .as_str()
                .ok_or("base64 decode: payload must be a base64 JSON string")?;
            let bytes = STANDARD
                .decode(s)
                .map_err(|e| format!("base64 decode: invalid base64 — {e}"))?;
            let text = String::from_utf8(bytes)
                .map_err(|e| format!("base64 decode: decoded bytes are not UTF-8 — {e}"))?;
            Ok(Value::String(text))
        }
        other => Err(format!("base64: unknown mode {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn csv_parse_with_header() {
        let out = csv(&json!({}), &json!("a,b\n1,2\n3,4"), "parse").unwrap();
        assert_eq!(out, json!([{"a":"1","b":"2"},{"a":"3","b":"4"}]));
    }

    #[test]
    fn csv_parse_no_header_and_separator() {
        let cfg = json!({"header": false, "separator": ";"});
        let out = csv(&cfg, &json!("1;2\n3;4"), "parse").unwrap();
        assert_eq!(out, json!([["1", "2"], ["3", "4"]]));
    }

    #[test]
    fn csv_round_trip_objects() {
        let rows = json!([{"a":"1","b":"2"},{"a":"3","b":"4"}]);
        let text = csv(&json!({}), &rows, "stringify").unwrap();
        let back = csv(&json!({}), &text, "parse").unwrap();
        assert_eq!(back, rows);
    }

    #[test]
    fn csv_stringify_union_of_keys() {
        let rows = json!([{"a":"1"},{"b":"2"}]);
        let text = csv(&json!({}), &rows, "stringify").unwrap();
        assert_eq!(text.as_str().unwrap(), "a,b\n1,\n,2\n");
    }

    #[test]
    fn csv_failures() {
        assert!(csv(&json!({}), &json!("a,b\n1,2,3"), "parse").is_err()); // ragged row
        assert!(csv(&json!({}), &json!([1, 2]), "parse").is_err()); // non-string payload
        assert!(csv(&json!({}), &json!({"a": 1}), "stringify").is_err()); // non-array payload
        assert!(csv(&json!({}), &json!("a"), "nope").is_err()); // unknown mode
    }

    #[test]
    fn yaml_round_trip() {
        let doc = "name: bob\nage: 3\ntags:\n  - x\n  - y\n";
        let parsed = yaml(&json!({}), &json!(doc), "parse").unwrap();
        assert_eq!(parsed, json!({"name": "bob", "age": 3, "tags": ["x", "y"]}));
        let text = yaml(&json!({}), &parsed, "stringify").unwrap();
        let reparsed = yaml(&json!({}), &text, "parse").unwrap();
        assert_eq!(reparsed, parsed);
    }

    #[test]
    fn yaml_failures() {
        assert!(yaml(&json!({}), &json!("a: [1, 2"), "parse").is_err()); // unclosed flow seq
        assert!(yaml(&json!({}), &json!({"a": 1}), "parse").is_err()); // non-string payload
        assert!(yaml(&json!({}), &json!("a"), "weird").is_err()); // unknown mode
    }

    #[test]
    fn base64_round_trip_string() {
        let enc = base64(&json!({}), &json!("hello"), "encode").unwrap();
        assert_eq!(enc.as_str().unwrap(), "aGVsbG8=");
        let dec = base64(&json!({}), &enc, "decode").unwrap();
        assert_eq!(dec, json!("hello"));
    }

    #[test]
    fn base64_encode_non_string() {
        let enc = base64(&json!({}), &json!({"a": 1}), "encode").unwrap();
        let dec = base64(&json!({}), &enc, "decode").unwrap();
        assert_eq!(dec.as_str().unwrap(), r#"{"a":1}"#);
    }

    #[test]
    fn base64_failures() {
        assert!(base64(&json!({}), &json!("not*base64*"), "decode").is_err()); // invalid base64
        let bad = STANDARD.encode([0xFF, 0xFE]); // valid base64, non-UTF-8 bytes
        assert!(base64(&json!({}), &json!(bad), "decode").is_err());
        assert!(base64(&json!({}), &json!(123), "decode").is_err()); // non-string payload
        assert!(base64(&json!({}), &json!("x"), "huh").is_err()); // unknown mode
    }
}
