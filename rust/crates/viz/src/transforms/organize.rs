//! The `organize` transformer — Grafana's `OrganizeFieldsTransformer` (viz transformations scope).
//! Per-frame field surgery: drop (`excludeByName`), keep-only (`includeByName`), reorder
//! (`indexByName`), rename (`renameByName`) — option shape verbatim. One responsibility: field
//! selection + ordering + naming. Pure: takes frames by value, returns reshaped frames; the row
//! count is unchanged (it edits columns, not rows) but we `relen` defensively.

use serde_json::Value;

use crate::frame::{Frame, Frames};

/// Apply `organize` to every frame. Order of operations mirrors Grafana: exclude/include selection,
/// then reorder by `indexByName`, then rename by `renameByName`.
pub fn apply(frames: Frames, options: &Value) -> Frames {
    frames.into_iter().map(|f| organize(f, options)).collect()
}

fn organize(frame: Frame, options: &Value) -> Frame {
    let exclude = options.get("excludeByName");
    let include = options.get("includeByName");
    let index = options.get("indexByName");
    let rename = options.get("renameByName");

    let include_any = include
        .and_then(Value::as_object)
        .map(|m| m.values().any(|v| v.as_bool() == Some(true)))
        .unwrap_or(false);

    // 1. Selection: drop excluded; if includeByName has any `true`, keep only those.
    let mut fields: Vec<_> = frame
        .fields
        .into_iter()
        .filter(|f| !bool_at(exclude, &f.name))
        .filter(|f| !include_any || bool_at(include, &f.name))
        .collect();

    // 2. Reorder by indexByName (ascending). Listed fields come first in index order; unlisted keep
    //    their relative order after the listed ones (Grafana's stable sort by index, default large).
    if let Some(idx) = index.and_then(Value::as_object) {
        let order_of = |name: &str| -> f64 {
            idx.get(name)
                .and_then(Value::as_f64)
                .unwrap_or(f64::INFINITY)
        };
        // Stable sort preserves relative order of equal (unlisted) keys.
        fields.sort_by(|a, b| {
            order_of(&a.name)
                .partial_cmp(&order_of(&b.name))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // 3. Rename via renameByName (empty target → keep original name).
    if let Some(map) = rename.and_then(Value::as_object) {
        for f in &mut fields {
            if let Some(new) = map.get(&f.name).and_then(Value::as_str) {
                if !new.is_empty() {
                    f.name = new.to_string();
                }
            }
        }
    }

    let mut out = Frame::new(fields).relen();
    out.ref_id = frame.ref_id;
    out.name = frame.name;
    out
}

/// Whether `name` maps to `true` in an optional `Record<string,bool>` option.
fn bool_at(map: Option<&Value>, name: &str) -> bool {
    map.and_then(Value::as_object)
        .and_then(|m| m.get(name))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Field;
    use serde_json::json;

    fn seeded() -> Frames {
        vec![Frame::new(vec![
            Field::new("a", vec![json!(1), json!(2)]),
            Field::new("b", vec![json!(3), json!(4)]),
            Field::new("c", vec![json!(5), json!(6)]),
        ])]
    }

    #[test]
    fn exclude_drops_named_fields() {
        let out = apply(seeded(), &json!({ "excludeByName": { "b": true } }));
        let names: Vec<&str> = out[0].fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a", "c"]);
    }

    #[test]
    fn include_keeps_only_selected() {
        let out = apply(
            seeded(),
            &json!({ "includeByName": { "a": true, "c": true } }),
        );
        let names: Vec<&str> = out[0].fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a", "c"]);
    }

    #[test]
    fn reorder_then_rename() {
        let out = apply(
            seeded(),
            &json!({ "indexByName": { "c": 0, "a": 1 }, "renameByName": { "c": "first" } }),
        );
        let names: Vec<&str> = out[0].fields.iter().map(|f| f.name.as_str()).collect();
        // c,a listed (0,1) then b unlisted; c renamed to "first".
        assert_eq!(names, vec!["first", "a", "b"]);
        assert_eq!(out[0].length, 2);
    }

    #[test]
    fn empty_options_passes_through() {
        let out = apply(seeded(), &json!({}));
        let names: Vec<&str> = out[0].fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }
}
