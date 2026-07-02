//! The one shared **field-path** helper for the data/JSON node pack (data-nodes scope, Risk 5 — "one
//! small shared field-path helper, not four bespoke matchers"). `change`/`select`/`switch`/`filter`
//! all address a `payload` sub-value by the **exact existing dot-path walker** the binding grammar
//! uses (data-nodes Open Q4, resolved: *exactly* the existing walker — dot-separated keys, numeric
//! array indices, missing → `null`; no wildcards/superset until a caller forces it).
//!
//! [`get`] reads (the `binding::walk_path` logic verbatim); [`set`]/[`delete`] mutate (the `change`
//! node's ordered ops). A `set` creates intermediate objects; a numeric segment into a missing/short
//! array is a no-op write (we never widen an array by index — Node-RED parity). Pure, no I/O.

use serde_json::{Map, Value};

/// Read a dot-path (`a.b.0.c`) out of `root`. A missing key, or an index into a non-array/non-object,
/// resolves to `null`. Array indices are numeric path segments. Verbatim the binding walker so a
/// `change`/`select` path and a `${steps.x.<path>}` binding address a value identically.
pub fn get(root: &Value, path: &str) -> Value {
    let mut cur = root;
    for seg in path.split('.') {
        cur = match cur {
            Value::Object(m) => match m.get(seg) {
                Some(v) => v,
                None => return Value::Null,
            },
            Value::Array(a) => match seg.parse::<usize>().ok().and_then(|i| a.get(i)) {
                Some(v) => v,
                None => return Value::Null,
            },
            _ => return Value::Null,
        };
    }
    cur.clone()
}

/// Whether a dot-path exists (distinct from "resolves to null" — a present `null` value exists).
pub fn has(root: &Value, path: &str) -> bool {
    let mut cur = root;
    for seg in path.split('.') {
        cur = match cur {
            Value::Object(m) => match m.get(seg) {
                Some(v) => v,
                None => return false,
            },
            Value::Array(a) => match seg.parse::<usize>().ok().and_then(|i| a.get(i)) {
                Some(v) => v,
                None => return false,
            },
            _ => return false,
        };
    }
    true
}

/// Set a dot-path in `root` to `val`, creating intermediate **objects** as needed. Setting a path
/// through a scalar replaces that scalar with an object (the last write wins on shape). A numeric
/// segment addressing an existing array slot writes in place; addressing past the end is a no-op (we
/// never widen an array by index — Node-RED parity). An empty path replaces the whole root.
pub fn set(root: &mut Value, path: &str, val: Value) {
    if path.is_empty() {
        *root = val;
        return;
    }
    let segs: Vec<&str> = path.split('.').collect();
    set_segs(root, &segs, val);
}

fn set_segs(cur: &mut Value, segs: &[&str], val: Value) {
    let (head, rest) = match segs.split_first() {
        Some(x) => x,
        None => {
            *cur = val;
            return;
        }
    };
    if rest.is_empty() {
        match cur {
            Value::Array(a) => {
                if let Some(i) = head.parse::<usize>().ok() {
                    if let Some(slot) = a.get_mut(i) {
                        *slot = val;
                    }
                }
            }
            _ => {
                let obj = ensure_object(cur);
                obj.insert((*head).to_string(), val);
            }
        }
        return;
    }
    // Descend, creating an object at `head` if the child is absent or a scalar.
    match cur {
        Value::Array(a) => {
            if let Some(slot) = head.parse::<usize>().ok().and_then(|i| a.get_mut(i)) {
                set_segs(slot, rest, val);
            }
        }
        _ => {
            let obj = ensure_object(cur);
            let child = obj.entry((*head).to_string()).or_insert(Value::Null);
            if !matches!(child, Value::Object(_) | Value::Array(_)) {
                *child = Value::Object(Map::new());
            }
            set_segs(child, rest, val);
        }
    }
}

/// Delete a dot-path from `root`. A missing path is a no-op. A numeric segment removes an array
/// element (shifting the rest); a key segment removes an object entry.
pub fn delete(root: &mut Value, path: &str) {
    let segs: Vec<&str> = path.split('.').collect();
    delete_segs(root, &segs);
}

fn delete_segs(cur: &mut Value, segs: &[&str]) {
    let (head, rest) = match segs.split_first() {
        Some(x) => x,
        None => return,
    };
    if rest.is_empty() {
        match cur {
            Value::Object(m) => {
                m.remove(*head);
            }
            Value::Array(a) => {
                if let Some(i) = head.parse::<usize>().ok() {
                    if i < a.len() {
                        a.remove(i);
                    }
                }
            }
            _ => {}
        }
        return;
    }
    match cur {
        Value::Object(m) => {
            if let Some(child) = m.get_mut(*head) {
                delete_segs(child, rest);
            }
        }
        Value::Array(a) => {
            if let Some(child) = head.parse::<usize>().ok().and_then(|i| a.get_mut(i)) {
                delete_segs(child, rest);
            }
        }
        _ => {}
    }
}

/// Coerce `cur` into an object in place (replacing a scalar/null), returning the map.
fn ensure_object(cur: &mut Value) -> &mut Map<String, Value> {
    if !matches!(cur, Value::Object(_)) {
        *cur = Value::Object(Map::new());
    }
    match cur {
        Value::Object(m) => m,
        _ => unreachable!("just set to object"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn get_walks_objects_and_arrays() {
        let v = json!({"a": {"b": [10, 20]}, "t": "x"});
        assert_eq!(get(&v, "a.b.1"), json!(20));
        assert_eq!(get(&v, "t"), json!("x"));
        assert_eq!(get(&v, "a.nope"), Value::Null);
        assert_eq!(get(&v, "a.b.9"), Value::Null);
    }

    #[test]
    fn has_distinguishes_present_null_from_missing() {
        let v = json!({"a": null});
        assert!(has(&v, "a"));
        assert!(!has(&v, "b"));
    }

    #[test]
    fn set_creates_intermediate_objects() {
        let mut v = json!({});
        set(&mut v, "a.b.c", json!(1));
        assert_eq!(v, json!({"a": {"b": {"c": 1}}}));
        // replace-through-scalar
        set(&mut v, "a.b", json!("flat"));
        assert_eq!(v, json!({"a": {"b": "flat"}}));
    }

    #[test]
    fn set_writes_array_slot_in_place_but_not_past_end() {
        let mut v = json!({"xs": [1, 2, 3]});
        set(&mut v, "xs.0", json!(9));
        assert_eq!(v, json!({"xs": [9, 2, 3]}));
        set(&mut v, "xs.5", json!(9)); // no-op, never widens
        assert_eq!(v, json!({"xs": [9, 2, 3]}));
    }

    #[test]
    fn delete_removes_object_key_and_array_elem() {
        let mut v = json!({"a": {"b": 1, "c": 2}, "xs": [1, 2, 3]});
        delete(&mut v, "a.b");
        assert_eq!(v, json!({"a": {"c": 2}, "xs": [1, 2, 3]}));
        delete(&mut v, "xs.1");
        assert_eq!(v, json!({"a": {"c": 2}, "xs": [1, 3]}));
        delete(&mut v, "nope.gone"); // no-op
    }

    #[test]
    fn empty_path_replaces_root() {
        let mut v = json!({"a": 1});
        set(&mut v, "", json!([1, 2]));
        assert_eq!(v, json!([1, 2]));
    }
}
