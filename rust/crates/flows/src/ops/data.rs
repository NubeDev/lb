//! The **data-transform** node pack (data-nodes scope) — eight pure, payload-in/payload-out shapers
//! that reuse the two shared helpers ([`super::path`], [`super::predicate`]) rather than growing
//! bespoke walkers/coercers (data-nodes Risk 5 — "one small shared helper, not four"). Every op is a
//! total function `Result<Value, String>`; `Ok` is the **new payload**, `Err` is a caller-facing
//! message. Nothing mutates its input in place — `change`/`map` clone first. The ops: `change`
//! (ordered `set`/`delete`/`move`/`copy`), `select` (keep chosen dot-paths), `merge` (deep-merge an
//! array of objects, last-writer-wins), `map` (a `change` op set per array element), `flatten` (nested
//! arrays by depth, or dot-join a nested object), `sort` (stable, lexical/numeric, by path or whole
//! element), `range` (linear scale between two ranges, optional clamp), `aggregate` (reduce an array →
//! `sum`/`min`/`max`/`mean`/`count`/`concat`).

use super::{path, predicate};
use serde_json::{Map, Value};

/// Apply an ordered op list to a **clone** of `payload`. `config = { "ops": [ {op, ...} ] }` where
/// each op is `set`/`delete`/`move`/`copy` (see module doc). Ops apply in array order; an unknown op
/// is skipped (a mistyped op never aborts the pipeline). A non-object payload is still cloned and
/// mutated — `path::set` coerces the root to an object when a sub-path set targets it.
pub fn change(config: &Value, payload: &Value) -> Result<Value, String> {
    let mut out = payload.clone();
    let ops = config.get("ops").and_then(|v| v.as_array());
    for op in ops.into_iter().flatten() {
        apply_op(&mut out, op);
    }
    Ok(out)
}

/// One `change`/`map` op against `root` (in place). Missing fields default sanely: a set with no
/// `value` sets `null`; a move/copy with no `from`/`to` is a no-op.
fn apply_op(root: &mut Value, op: &Value) {
    let kind = op.get("op").and_then(|v| v.as_str()).unwrap_or("");
    let s = |k: &str| op.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    match kind {
        "set" => path::set(
            root,
            &s("path"),
            op.get("value").cloned().unwrap_or(Value::Null),
        ),
        "delete" => path::delete(root, &s("path")),
        "copy" => {
            let (from, to) = (s("from"), s("to"));
            if !from.is_empty() && !to.is_empty() {
                path::set(root, &to, path::get(root, &from));
            }
        }
        "move" => {
            let (from, to) = (s("from"), s("to"));
            if !from.is_empty() && !to.is_empty() {
                path::set(root, &to, path::get(root, &from));
                path::delete(root, &from);
            }
        }
        _ => {}
    }
}

/// Project `payload` down to the chosen dot-paths → a fresh object. `config = { "paths": [...] }`.
/// A path present in the source (per `path::has`, so a present `null` is kept) is written into the
/// new object at the **same** path; missing paths are skipped.
pub fn select(config: &Value, payload: &Value) -> Result<Value, String> {
    let mut out = Value::Object(Map::new());
    let paths = config.get("paths").and_then(|v| v.as_array());
    for p in paths.into_iter().flatten() {
        let Some(p) = p.as_str() else { continue };
        if path::has(payload, p) {
            path::set(&mut out, p, path::get(payload, p));
        }
    }
    Ok(out)
}

/// Deep-merge an **array of objects** into one object, last-writer-wins on scalar conflict; nested
/// objects merge recursively. A non-object element is skipped. Non-array payload → `Err`.
pub fn merge(payload: &Value) -> Result<Value, String> {
    let arr = payload.as_array().ok_or("merge expects an array payload")?;
    let mut out = Map::new();
    for elem in arr {
        if let Value::Object(m) = elem {
            for (k, v) in m {
                merge_into(&mut out, k, v);
            }
        }
    }
    Ok(Value::Object(out))
}

/// Merge one `key: val` into `dst`: recurse when both existing and incoming are objects, else the
/// incoming value wins (last-writer-wins).
fn merge_into(dst: &mut Map<String, Value>, key: &str, val: &Value) {
    match (dst.get_mut(key), val) {
        (Some(Value::Object(existing)), Value::Object(incoming)) => {
            for (k, v) in incoming {
                merge_into(existing, k, v);
            }
        }
        _ => {
            dst.insert(key.to_string(), val.clone());
        }
    }
}

/// Apply a `change`-style op set to **every** element of an array `payload`. Non-array payload →
/// `Err`. `config = { "ops": [...] }` (same grammar as [`change`]). Returns the new array.
pub fn map(config: &Value, payload: &Value) -> Result<Value, String> {
    let arr = payload.as_array().ok_or("map expects an array payload")?;
    let ops = config.get("ops").and_then(|v| v.as_array());
    let out: Vec<Value> = arr
        .iter()
        .map(|elem| {
            let mut e = elem.clone();
            for op in ops.into_iter().flatten() {
                apply_op(&mut e, op);
            }
            e
        })
        .collect();
    Ok(Value::Array(out))
}

/// Flatten nested arrays or dot-join a nested object. An **array** flattens `config.depth` levels
/// (absent/`<=0` → fully/deep); an **object** produces a flat object with dot-joined keys (depth is
/// ignored — always full); a scalar/null → `Err`.
pub fn flatten(config: &Value, payload: &Value) -> Result<Value, String> {
    match payload {
        Value::Array(a) => {
            let depth = config.get("depth").and_then(|v| v.as_i64()).unwrap_or(0);
            let depth = if depth <= 0 { i64::MAX } else { depth };
            let mut out = Vec::new();
            flatten_array(a, depth, &mut out);
            Ok(Value::Array(out))
        }
        Value::Object(_) => {
            let mut out = Map::new();
            flatten_object(payload, "", &mut out);
            Ok(Value::Object(out))
        }
        _ => Err("flatten expects an array or object payload".into()),
    }
}

/// Append `arr`'s elements to `out`, recursing into nested arrays up to `depth` more levels.
fn flatten_array(arr: &[Value], depth: i64, out: &mut Vec<Value>) {
    for elem in arr {
        match elem {
            Value::Array(inner) if depth > 0 => flatten_array(inner, depth - 1, out),
            other => out.push(other.clone()),
        }
    }
}

/// Walk a nested object into `out` with dot-joined keys; a nested array stops the walk (kept whole).
fn flatten_object(val: &Value, prefix: &str, out: &mut Map<String, Value>) {
    match val {
        Value::Object(m) if !m.is_empty() => {
            for (k, v) in m {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_object(v, &key, out);
            }
        }
        other => {
            out.insert(prefix.to_string(), other.clone());
        }
    }
}

/// Stable-sort an array `payload`. `config = { "path"?, "order"?: "asc"|"desc", "numeric"?: bool }`.
/// With `path`, compare `path::get(elem, path)`; else the whole element. `numeric` compares via
/// `predicate::as_f64` (non-numeric sorts last); otherwise lexical by JSON string. Non-array → `Err`.
pub fn sort(config: &Value, payload: &Value) -> Result<Value, String> {
    let arr = payload.as_array().ok_or("sort expects an array payload")?;
    let field = config.get("path").and_then(|v| v.as_str());
    let numeric = config
        .get("numeric")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let desc = config.get("order").and_then(|v| v.as_str()) == Some("desc");
    let key = |e: &Value| -> Value {
        match field {
            Some(p) => path::get(e, p),
            None => e.clone(),
        }
    };
    let mut out = arr.clone();
    out.sort_by(|a, b| {
        let (ka, kb) = (key(a), key(b));
        let ord = if numeric {
            // Non-numeric sorts last (ascending); `None`s compare equal, keeping them stable.
            match (predicate::as_f64(&ka), predicate::as_f64(&kb)) {
                (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        } else {
            ka.to_string().cmp(&kb.to_string())
        };
        if desc {
            ord.reverse()
        } else {
            ord
        }
    });
    Ok(Value::Array(out))
}

/// Linearly scale a numeric `payload` from `[inMin,inMax]` to `[outMin,outMax]`. All four bounds are
/// required (missing → `Err`); `inMax == inMin` is degenerate → `Err`; a non-numeric payload → `Err`.
/// With `clamp`, the **result** is clamped to `[min(outMin,outMax), max(...)]`. Returns a JSON f64.
pub fn range(config: &Value, payload: &Value) -> Result<Value, String> {
    let bound = |k: &str| {
        config
            .get(k)
            .and_then(predicate::as_f64)
            .ok_or_else(|| format!("range missing numeric bound '{k}'"))
    };
    let (in_min, in_max) = (bound("inMin")?, bound("inMax")?);
    let (out_min, out_max) = (bound("outMin")?, bound("outMax")?);
    if in_max == in_min {
        return Err("range degenerate: inMax == inMin".into());
    }
    let x = predicate::as_f64(payload).ok_or("range expects a numeric payload")?;
    let mut y = out_min + (x - in_min) * (out_max - out_min) / (in_max - in_min);
    if config
        .get("clamp")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        let (lo, hi) = (out_min.min(out_max), out_min.max(out_max));
        y = y.clamp(lo, hi);
    }
    Ok(Value::from(y))
}

/// Reduce an array `payload` to a scalar. `config = { "op", "path"?, "sep"? }`. With `path`, pull
/// `path::get(elem, path)` from each element, else the element itself. `sum`/`min`/`max`/`mean` use
/// `predicate::as_f64` and skip non-numerics; `count` is the element count; `concat` joins rendered
/// elements by `sep` (default `""`). Empty array: `count`→0, `sum`→0, `concat`→"", `mean`/`min`/`max`→
/// `Err`. Non-array payload → `Err`.
pub fn aggregate(config: &Value, payload: &Value) -> Result<Value, String> {
    let arr = payload
        .as_array()
        .ok_or("aggregate expects an array payload")?;
    let op = config.get("op").and_then(|v| v.as_str()).unwrap_or("");
    let field = config.get("path").and_then(|v| v.as_str());
    let pick = |e: &Value| -> Value {
        match field {
            Some(p) => path::get(e, p),
            None => e.clone(),
        }
    };
    match op {
        "count" => Ok(Value::from(arr.len())),
        "concat" => {
            let sep = config.get("sep").and_then(|v| v.as_str()).unwrap_or("");
            let parts: Vec<String> = arr.iter().map(|e| render(&pick(e))).collect();
            Ok(Value::from(parts.join(sep)))
        }
        "sum" | "min" | "max" | "mean" => {
            let nums: Vec<f64> = arr
                .iter()
                .filter_map(|e| predicate::as_f64(&pick(e)))
                .collect();
            match op {
                "sum" => Ok(Value::from(nums.iter().sum::<f64>())),
                "min" => nums
                    .iter()
                    .cloned()
                    .fold(None, |a: Option<f64>, x| Some(a.map_or(x, |m| m.min(x))))
                    .map(Value::from)
                    .ok_or_else(|| "aggregate min of empty array".into()),
                "max" => nums
                    .iter()
                    .cloned()
                    .fold(None, |a: Option<f64>, x| Some(a.map_or(x, |m| m.max(x))))
                    .map(Value::from)
                    .ok_or_else(|| "aggregate max of empty array".into()),
                _ => {
                    if nums.is_empty() {
                        Err("aggregate mean of empty array".into())
                    } else {
                        Ok(Value::from(nums.iter().sum::<f64>() / nums.len() as f64))
                    }
                }
            }
        }
        other => Err(format!("aggregate unknown op '{other}'")),
    }
}

/// Render a value to a string for `concat` — a string renders bare (not JSON-quoted), everything else
/// via its JSON form.
fn render(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn change_applies_ops_in_order() {
        let cfg = json!({"ops": [
            {"op": "set", "path": "a.b", "value": 1},
            {"op": "copy", "from": "a.b", "to": "c"},
            {"op": "move", "from": "c", "to": "d"},
            {"op": "delete", "path": "a.b"},
        ]});
        assert_eq!(change(&cfg, &json!({})).unwrap(), json!({"a": {}, "d": 1}));
        // non-object payload is coerced by a sub-path set
        assert_eq!(
            change(
                &json!({"ops": [{"op": "set", "path": "x", "value": 2}]}),
                &json!(7)
            )
            .unwrap(),
            json!({"x": 2})
        );
        // unknown op is skipped, not an error
        assert_eq!(
            change(&json!({"ops": [{"op": "bogus"}]}), &json!({"k": 1})).unwrap(),
            json!({"k": 1})
        );
    }

    #[test]
    fn select_keeps_only_chosen_paths() {
        let p = json!({"a": 1, "b": {"c": 2, "d": 3}, "e": null});
        assert_eq!(
            select(&json!({"paths": ["a", "b.c", "e", "missing"]}), &p).unwrap(),
            json!({"a": 1, "b": {"c": 2}, "e": null})
        );
        assert_eq!(select(&json!({"paths": []}), &p).unwrap(), json!({}));
    }

    #[test]
    fn merge_deep_last_writer_wins() {
        let p = json!([{"a": 1, "n": {"x": 1}}, {"a": 2, "n": {"y": 2}}, "skip-me"]);
        assert_eq!(merge(&p).unwrap(), json!({"a": 2, "n": {"x": 1, "y": 2}}));
        assert_eq!(merge(&json!([])).unwrap(), json!({}));
        assert!(merge(&json!({"not": "array"})).is_err());
    }

    #[test]
    fn map_applies_ops_per_element() {
        let cfg = json!({"ops": [{"op": "set", "path": "seen", "value": true}]});
        assert_eq!(
            map(&cfg, &json!([{"id": 1}, {"id": 2}])).unwrap(),
            json!([{"id": 1, "seen": true}, {"id": 2, "seen": true}])
        );
        assert!(map(&cfg, &json!("nope")).is_err());
    }

    #[test]
    fn flatten_arrays_by_depth_and_object_keys() {
        assert_eq!(
            flatten(&json!({}), &json!([1, [2, [3, [4]]]])).unwrap(),
            json!([1, 2, 3, 4])
        );
        assert_eq!(
            flatten(&json!({"depth": 1}), &json!([1, [2, [3]]])).unwrap(),
            json!([1, 2, [3]])
        );
        assert_eq!(
            flatten(&json!({}), &json!({"a": {"b": 1, "c": {"d": 2}}, "e": 3})).unwrap(),
            json!({"a.b": 1, "a.c.d": 2, "e": 3})
        );
        assert!(flatten(&json!({}), &json!(5)).is_err());
    }

    #[test]
    fn sort_lexical_numeric_and_by_path() {
        assert_eq!(
            sort(&json!({}), &json!([3, 1, 2])).unwrap(),
            json!([1, 2, 3])
        ); // lexical here == numeric
        assert_eq!(
            sort(
                &json!({"numeric": true, "order": "desc"}),
                &json!(["10", "2", "1"])
            )
            .unwrap(),
            json!(["10", "2", "1"])
        );
        assert_eq!(
            sort(
                &json!({"path": "v", "numeric": true}),
                &json!([{"v": 3}, {"v": 1}, {"v": 2}])
            )
            .unwrap(),
            json!([{"v": 1}, {"v": 2}, {"v": 3}])
        );
        assert!(sort(&json!({}), &json!("nope")).is_err());
    }

    #[test]
    fn sort_numeric_puts_non_numeric_last_stably() {
        assert_eq!(
            sort(&json!({"numeric": true}), &json!([2, "x", 1, "y"])).unwrap(),
            json!([1, 2, "x", "y"])
        );
    }

    #[test]
    fn range_scales_and_clamps() {
        let cfg = json!({"inMin": 0, "inMax": 10, "outMin": 0, "outMax": 100});
        assert_eq!(range(&cfg, &json!(5)).unwrap(), json!(50.0));
        let clamp = json!({"inMin": 0, "inMax": 10, "outMin": 0, "outMax": 100, "clamp": true});
        assert_eq!(range(&clamp, &json!(20)).unwrap(), json!(100.0));
        // degenerate + missing bound + non-numeric payload
        assert!(range(
            &json!({"inMin": 5, "inMax": 5, "outMin": 0, "outMax": 1}),
            &json!(5)
        )
        .is_err());
        assert!(range(&json!({"inMin": 0, "inMax": 10, "outMin": 0}), &json!(5)).is_err());
        assert!(range(&cfg, &json!("nope")).is_err());
    }

    #[test]
    fn aggregate_reduces_to_scalar() {
        let xs = json!([1, 2, 3, "skip", "4"]);
        assert_eq!(aggregate(&json!({"op": "sum"}), &xs).unwrap(), json!(10.0)); // "4" coerces
        assert_eq!(aggregate(&json!({"op": "count"}), &xs).unwrap(), json!(5));
        assert_eq!(
            aggregate(&json!({"op": "max"}), &json!([1, 5, 3])).unwrap(),
            json!(5.0)
        );
        assert_eq!(
            aggregate(&json!({"op": "mean"}), &json!([2, 4])).unwrap(),
            json!(3.0)
        );
    }

    #[test]
    fn aggregate_path_concat_and_empty_contracts() {
        let people = json!([{"n": "a"}, {"n": "b"}]);
        assert_eq!(
            aggregate(&json!({"op": "concat", "path": "n", "sep": "-"}), &people).unwrap(),
            json!("a-b")
        );
        assert_eq!(
            aggregate(&json!({"op": "count"}), &json!([])).unwrap(),
            json!(0)
        );
        assert_eq!(
            aggregate(&json!({"op": "sum"}), &json!([])).unwrap(),
            json!(0.0)
        );
        assert_eq!(
            aggregate(&json!({"op": "concat"}), &json!([])).unwrap(),
            json!("")
        );
        assert!(aggregate(&json!({"op": "mean"}), &json!([])).is_err());
        assert!(aggregate(&json!({"op": "min"}), &json!([])).is_err());
        assert!(aggregate(&json!({"op": "sum"}), &json!("nope")).is_err());
    }
}
