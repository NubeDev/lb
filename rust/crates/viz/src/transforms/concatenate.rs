//! The `concatenate` transformer — Grafana's `ConcatenateTransformer` (viz grafana-parity scope,
//! tranche 2a). Horizontal concat: every frame's fields side by side in ONE frame, padded to the
//! longest frame (a short field reads `Null` past its end — ragged-honest). Options verbatim:
//! `{ frameNameMode?: "field"|"label"|"drop", frameNameLabel? }` —
//!   - `field` (Grafana's default): each field is renamed `{frame name} · {field name}` so
//!     same-named fields from different frames stay distinguishable.
//!   - `label`: the frame name lands as a label (`frameNameLabel`, default `"frame"`) instead.
//!   - `drop`: names untouched.
//! A frame with no `name` contributes its `refId` (else nothing to say — name untouched).

use serde_json::Value;

use crate::frame::{Frame, Frames};

/// Apply `concatenate`: N frames → 1. Zero/one frame passes through.
pub fn apply(frames: Frames, options: &Value) -> Frames {
    if frames.len() <= 1 {
        return frames;
    }
    let mode = options
        .get("frameNameMode")
        .and_then(Value::as_str)
        .unwrap_or("field");
    let label_key = options
        .get("frameNameLabel")
        .and_then(Value::as_str)
        .unwrap_or("frame");

    let rows = frames.iter().map(|f| f.length).max().unwrap_or(0);
    let first_ref = frames[0].ref_id.clone();
    let mut fields = Vec::new();
    for frame in frames {
        let frame_name = if frame.name.is_empty() {
            frame.ref_id.clone()
        } else {
            frame.name.clone()
        };
        for mut field in frame.fields {
            match mode {
                "field" if !frame_name.is_empty() => {
                    field.name = format!("{frame_name} · {}", field.name);
                }
                "label" => {
                    field
                        .labels
                        .insert(label_key.to_string(), Value::from(frame_name.clone()));
                }
                _ => {}
            }
            // Pad to the longest frame so every column is row-addressable.
            field.values.resize(rows, Value::Null);
            fields.push(field);
        }
    }
    let mut out = Frame::new(fields).relen();
    out.ref_id = first_ref;
    vec![out]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Field;
    use serde_json::json;

    fn seeded() -> Frames {
        let mut a = Frame::new(vec![Field::new("v", vec![json!(1), json!(2)])]);
        a.ref_id = "A".into();
        let mut b = Frame::new(vec![Field::new("v", vec![json!(9)])]);
        b.ref_id = "B".into();
        b.name = "fryer".into();
        vec![a, b]
    }

    #[test]
    fn field_mode_prefixes_names_and_pads_short_frames() {
        let out = apply(seeded(), &json!({}));
        assert_eq!(out.len(), 1);
        let names: Vec<&str> = out[0].fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["A · v", "fryer · v"]);
        assert_eq!(
            out[0].fields[1].values,
            vec![json!(9), json!(null)],
            "short frame padded with honest nulls"
        );
        assert_eq!(out[0].length, 2);
    }

    #[test]
    fn label_mode_tags_fields_instead_of_renaming() {
        let out = apply(
            seeded(),
            &json!({ "frameNameMode": "label", "frameNameLabel": "src" }),
        );
        let names: Vec<&str> = out[0].fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["v", "v"]);
        assert_eq!(out[0].fields[1].labels.get("src"), Some(&json!("fryer")));
    }

    #[test]
    fn drop_mode_keeps_names_and_single_frame_passes_through() {
        let out = apply(seeded(), &json!({ "frameNameMode": "drop" }));
        assert_eq!(out[0].fields[0].name, "v");
        let single = vec![Frame::new(vec![Field::new("v", vec![json!(1)])])];
        assert_eq!(apply(single.clone(), &json!({})), single);
    }
}
