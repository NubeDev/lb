//! `xml` — the text ↔ structured-value converter with a small, round-trippable convention
//! (data-nodes Parse). element → object; attributes under `@name`; text under `#text`; repeated
//! same-named children → an array; the single top-level key = the root element. Not a full XML
//! binding (namespaces are stripped to local names; CDATA/comments/PIs are ignored) — enough for the
//! flow boundary, and it round-trips. Malformed XML FAILS the node (json-node parity). Pure.

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer, XmlVersion};
use serde_json::{Map, Value};

use super::cell_string;

/// `parse` decodes an XML JSON string using the convention above; `stringify` reverses a
/// single-root-key object into an XML JSON string. Malformed XML, a non-object/multi-root `stringify`
/// payload, or an unknown mode is `Err`.
pub fn xml(_config: &Value, payload: &Value, mode: &str) -> Result<Value, String> {
    match mode {
        "parse" => {
            let text = payload
                .as_str()
                .ok_or("xml parse: payload must be a JSON string of XML")?;
            let mut reader = Reader::from_str(text);
            reader.config_mut().trim_text(true);
            let (name, val) = parse_element(&mut reader)?;
            let mut root = Map::new();
            root.insert(name, val);
            Ok(Value::Object(root))
        }
        "stringify" => {
            let obj = payload
                .as_object()
                .ok_or("xml stringify: payload must be a single-root-key object")?;
            if obj.len() != 1 {
                return Err("xml stringify: object must have exactly one root key".into());
            }
            let (name, val) = obj.iter().next().unwrap();
            let mut writer = Writer::new(Vec::new());
            write_element(&mut writer, name, val)?;
            let s = String::from_utf8(writer.into_inner()).map_err(|e| e.to_string())?;
            Ok(Value::String(s))
        }
        other => Err(format!("xml: unknown mode {other:?}")),
    }
}

/// Read the root element (the reader is positioned before its `Start`) into a value + its name.
fn parse_element(reader: &mut Reader<&[u8]>) -> Result<(String, Value), String> {
    loop {
        match reader.read_event().map_err(|e| format!("xml parse: {e}"))? {
            Event::Start(ref e) => {
                let name = local_name(e)?;
                let val = parse_children(reader, e)?;
                return Ok((name, val));
            }
            Event::Empty(ref e) => {
                let name = local_name(e)?;
                return Ok((name, Value::Object(element_map(e)?)));
            }
            Event::Eof => return Err("xml parse: empty document".into()),
            _ => continue,
        }
    }
}

/// Collect an element's attributes, text and child elements until its matching `End`.
fn parse_children(reader: &mut Reader<&[u8]>, start: &BytesStart) -> Result<Value, String> {
    let mut obj = element_map(start)?;
    let mut text = String::new();
    loop {
        match reader.read_event().map_err(|e| format!("xml parse: {e}"))? {
            Event::Start(ref e) => {
                let name = local_name(e)?;
                let child = parse_children(reader, e)?;
                insert_child(&mut obj, name, child);
            }
            Event::Empty(ref e) => {
                let name = local_name(e)?;
                let child = Value::Object(element_map(e)?);
                insert_child(&mut obj, name, child);
            }
            Event::Text(e) => {
                text.push_str(
                    &e.xml_content(XmlVersion::Implicit1_0)
                        .map_err(|e| format!("xml parse: {e}"))?,
                );
            }
            Event::End(_) => break,
            Event::Eof => return Err("xml parse: unexpected end of document".into()),
            _ => continue,
        }
    }
    if !text.is_empty() {
        obj.insert("#text".into(), Value::String(text));
    }
    // A bare text-only element (no attrs, no children) collapses to its string.
    if obj.len() == 1 {
        if let Some(Value::String(s)) = obj.get("#text") {
            return Ok(Value::String(s.clone()));
        }
    }
    Ok(Value::Object(obj))
}

/// A fresh child object seeded with the element's `@`-prefixed attributes.
fn element_map(e: &BytesStart) -> Result<Map<String, Value>, String> {
    let mut obj = Map::new();
    for attr in e.attributes() {
        let attr = attr.map_err(|e| format!("xml parse: bad attribute — {e}"))?;
        let key = String::from_utf8(attr.key.local_name().as_ref().to_vec())
            .map_err(|e| e.to_string())?;
        let val = attr
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|e| format!("xml parse: {e}"))?
            .into_owned();
        obj.insert(format!("@{key}"), Value::String(val));
    }
    Ok(obj)
}

/// Insert a child under `name`, promoting to an array on the second same-named sibling.
fn insert_child(obj: &mut Map<String, Value>, name: String, child: Value) {
    match obj.get_mut(&name) {
        Some(Value::Array(arr)) => arr.push(child),
        Some(existing) => {
            let prev = existing.take();
            *existing = Value::Array(vec![prev, child]);
        }
        None => {
            obj.insert(name, child);
        }
    }
}

/// The element's local (namespace-stripped) name as a `String`.
fn local_name(e: &BytesStart) -> Result<String, String> {
    String::from_utf8(e.local_name().as_ref().to_vec()).map_err(|e| e.to_string())
}

/// Write one `name` element for `val`, applying the convention in reverse.
fn write_element(writer: &mut Writer<Vec<u8>>, name: &str, val: &Value) -> Result<(), String> {
    match val {
        Value::Array(items) => {
            for item in items {
                write_element(writer, name, item)?;
            }
            Ok(())
        }
        Value::Object(map) => {
            let mut start = BytesStart::new(name);
            for (k, v) in map {
                if let Some(attr) = k.strip_prefix('@') {
                    start.push_attribute((attr, cell_string(Some(v)).as_str()));
                }
            }
            writer
                .write_event(Event::Start(start))
                .map_err(|e| e.to_string())?;
            if let Some(Value::String(t)) = map.get("#text") {
                writer
                    .write_event(Event::Text(BytesText::new(t)))
                    .map_err(|e| e.to_string())?;
            }
            for (k, v) in map {
                if !k.starts_with('@') && k != "#text" {
                    write_element(writer, k, v)?;
                }
            }
            writer
                .write_event(Event::End(BytesEnd::new(name)))
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        other => {
            writer
                .write_event(Event::Start(BytesStart::new(name)))
                .map_err(|e| e.to_string())?;
            writer
                .write_event(Event::Text(BytesText::new(&cell_string(Some(other)))))
                .map_err(|e| e.to_string())?;
            writer
                .write_event(Event::End(BytesEnd::new(name)))
                .map_err(|e| e.to_string())?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn xml_parse_convention() {
        let doc = r#"<r><a>1</a><a>2</a><b x="y">t</b></r>"#;
        let out = xml(&json!({}), &json!(doc), "parse").unwrap();
        assert_eq!(
            out,
            json!({"r": {"a": ["1", "2"], "b": {"@x": "y", "#text": "t"}}})
        );
    }

    #[test]
    fn xml_round_trip() {
        let doc = r#"<r><a>1</a><a>2</a><b x="y">t</b></r>"#;
        let parsed = xml(&json!({}), &json!(doc), "parse").unwrap();
        let text = xml(&json!({}), &parsed, "stringify").unwrap();
        // Reparse the serialized form: the structure must be identical.
        let reparsed = xml(&json!({}), &text, "parse").unwrap();
        assert_eq!(reparsed, parsed);
    }

    #[test]
    fn xml_failures() {
        assert!(xml(&json!({}), &json!("<r><a></r>"), "parse").is_err()); // unclosed tag
        assert!(xml(&json!({}), &json!({"r": 1}), "parse").is_err()); // non-string payload
        assert!(xml(&json!({}), &json!([1, 2]), "stringify").is_err()); // not a single-root object
        assert!(xml(&json!({}), &json!({"a": 1, "b": 2}), "stringify").is_err()); // two roots
        assert!(xml(&json!({}), &json!("x"), "bogus").is_err()); // unknown mode
    }
}
