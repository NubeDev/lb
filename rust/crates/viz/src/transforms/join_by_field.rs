//! `joinByField` / `seriesToColumns` (viz transformations scope, "Adopt Grafana's transformation
//! model verbatim"). Join ALL input frames into ONE frame on a shared key column: outer (the sorted
//! union of key values, default) or inner (only keys present in every frame). Each non-key field
//! becomes a column, looked up by key per frame (Null where a frame lacks that key). Pure +
//! deterministic. One responsibility: the field-join.

use serde_json::Value;

use crate::frame::{Field, FieldType, Frame, Frames};

/// Grafana `JoinByFieldOptions { byField?, mode? }`. `mode` is `"outer"` (default) or `"inner"`.
fn parse(options: &Value) -> (Option<String>, bool) {
    let by_field = options
        .get("byField")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let inner = options.get("mode").and_then(Value::as_str) == Some("inner");
    (by_field, inner)
}

/// Pick the join key for `frames`: explicit `byField`, else the first Time field present in every
/// frame, else the first field name shared by all frames. Returns `None` when no shared key exists.
fn pick_key(frames: &Frames, by_field: Option<&str>) -> Option<String> {
    if let Some(name) = by_field {
        return Some(name.to_string());
    }
    let shared_in_all = |name: &str| frames.iter().all(|f| f.field(name).is_some());
    // First Time field shared by all.
    for f in frames {
        for field in &f.fields {
            if field.ty == FieldType::Time && shared_in_all(&field.name) {
                return Some(field.name.clone());
            }
        }
    }
    // Else first field shared by all (first frame's order).
    for field in &frames[0].fields {
        if shared_in_all(&field.name) {
            return Some(field.name.clone());
        }
    }
    None
}

/// Key as a stable string for grouping (canonical: numbers compare numerically for sort, but identity
/// is by JSON repr so a key value maps consistently).
fn key_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn apply(frames: Frames, options: &Value) -> Frames {
    if frames.len() <= 1 {
        return frames;
    }
    let (by_field, inner) = parse(options);
    let key_name = match pick_key(&frames, by_field.as_deref()) {
        Some(k) => k,
        None => return frames,
    };

    // Each frame must have the key column to participate.
    let participating: Vec<&Frame> = frames
        .iter()
        .filter(|f| f.field(&key_name).is_some())
        .collect();
    if participating.is_empty() {
        return frames;
    }

    // Collect per-frame key→row maps and a canonical key value for output ordering.
    let mut key_order: Vec<String> = Vec::new();
    let mut key_value: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    let mut per_frame_keys: Vec<std::collections::HashSet<String>> = Vec::new();
    let mut per_frame_index: Vec<std::collections::HashMap<String, usize>> = Vec::new();

    for f in &participating {
        let kf = f.field(&key_name).unwrap();
        let mut set = std::collections::HashSet::new();
        let mut idx = std::collections::HashMap::new();
        for row in 0..f.length {
            let v = kf.at(row);
            let ks = key_str(&v);
            if !key_value.contains_key(&ks) {
                key_value.insert(ks.clone(), v.clone());
                key_order.push(ks.clone());
            }
            if set.insert(ks.clone()) {
                idx.insert(ks, row);
            }
        }
        per_frame_keys.push(set);
        per_frame_index.push(idx);
    }

    // Determine the output key set.
    let keys: Vec<String> = if inner {
        key_order
            .into_iter()
            .filter(|k| per_frame_keys.iter().all(|s| s.contains(k)))
            .collect()
    } else {
        key_order
    };

    // Sort keys: numeric when the canonical value is numeric, else lexical (nulls/strings stable).
    let mut sorted_keys = keys;
    sorted_keys.sort_by(|a, b| {
        let va = key_value.get(a);
        let vb = key_value.get(b);
        match (va.and_then(Value::as_f64), vb.and_then(Value::as_f64)) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
            _ => a.cmp(b),
        }
    });

    // Build the key column (type from the first participating frame's key field).
    let key_ty = participating[0].field(&key_name).unwrap().ty;
    let key_vals: Vec<Value> = sorted_keys
        .iter()
        .map(|k| key_value.get(k).cloned().unwrap_or(Value::Null))
        .collect();
    let mut out_fields: Vec<Field> = vec![Field::typed(&key_name, key_ty, key_vals)];

    // Track used names to disambiguate duplicates.
    let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    used_names.insert(key_name.clone());

    for (fi, f) in participating.iter().enumerate() {
        for field in &f.fields {
            if field.name == key_name {
                continue;
            }
            let mut name = field.name.clone();
            if used_names.contains(&name) {
                let suffix = if f.ref_id.is_empty() {
                    fi.to_string()
                } else {
                    f.ref_id.clone()
                };
                name = format!("{} {}", field.name, suffix);
                // Still collide? append index.
                while used_names.contains(&name) {
                    name = format!("{name} {fi}");
                }
            }
            used_names.insert(name.clone());

            let idx = &per_frame_index[fi];
            let vals: Vec<Value> = sorted_keys
                .iter()
                .map(|k| idx.get(k).map(|&r| field.at(r)).unwrap_or(Value::Null))
                .collect();
            out_fields.push(Field::typed(name, field.ty, vals));
        }
    }

    let mut out = Frame::new(out_fields);
    out.ref_id = participating[0].ref_id.clone();
    vec![out.relen()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn frame(ref_id: &str, time: Vec<i64>, val_name: &str, vals: Vec<Value>) -> Frame {
        let mut f = Frame::new(vec![
            Field::typed(
                "time",
                FieldType::Time,
                time.into_iter().map(|t| json!(t)).collect(),
            ),
            Field::new(val_name, vals),
        ]);
        f.ref_id = ref_id.into();
        f
    }

    #[test]
    fn outer_join_unions_keys() {
        let a = frame("A", vec![1, 2], "a", vec![json!(10), json!(20)]);
        let b = frame("B", vec![2, 3], "b", vec![json!(200), json!(300)]);
        let out = apply(vec![a, b], &json!({"byField": "time"}));
        assert_eq!(out.len(), 1);
        let f = &out[0];
        // keys 1,2,3
        assert_eq!(f.length, 3);
        assert_eq!(
            f.field("time").unwrap().values,
            vec![json!(1), json!(2), json!(3)]
        );
        assert_eq!(
            f.field("a").unwrap().values,
            vec![json!(10), json!(20), Value::Null]
        );
        assert_eq!(
            f.field("b").unwrap().values,
            vec![Value::Null, json!(200), json!(300)]
        );
    }

    #[test]
    fn inner_join_keeps_only_shared_keys() {
        let a = frame("A", vec![1, 2], "a", vec![json!(10), json!(20)]);
        let b = frame("B", vec![2, 3], "b", vec![json!(200), json!(300)]);
        let out = apply(vec![a, b], &json!({"byField": "time", "mode": "inner"}));
        let f = &out[0];
        assert_eq!(f.length, 1);
        assert_eq!(f.field("time").unwrap().values, vec![json!(2)]);
        assert_eq!(f.field("a").unwrap().values, vec![json!(20)]);
        assert_eq!(f.field("b").unwrap().values, vec![json!(200)]);
    }

    #[test]
    fn duplicate_field_names_disambiguated() {
        let a = frame("A", vec![1], "value", vec![json!(1)]);
        let b = frame("B", vec![1], "value", vec![json!(2)]);
        let out = apply(vec![a, b], &json!({"byField": "time"}));
        let f = &out[0];
        assert!(f.field("value").is_some());
        assert!(f.field("value B").is_some());
    }

    #[test]
    fn single_frame_passes_through() {
        let a = frame("A", vec![1, 2], "a", vec![json!(10), json!(20)]);
        let out = apply(vec![a.clone()], &json!({"byField": "time"}));
        assert_eq!(out, vec![a]);
    }

    #[test]
    fn empty_value_columns_stay_null_not_zero() {
        // B has no numeric overlap at key 1 → must be Null, never a fabricated 0.
        let a = frame("A", vec![1, 2], "a", vec![json!(10), json!(20)]);
        let b = frame("B", vec![2], "b", vec![json!(200)]);
        let out = apply(vec![a, b], &json!({"byField": "time"}));
        let f = &out[0];
        assert_eq!(f.field("b").unwrap().at(0), Value::Null);
    }
}
