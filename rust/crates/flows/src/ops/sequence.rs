//! The **sequence** ops — `split` / `join` — and the shared `parts` metadata contract (data-nodes
//! Decision 15, resolving Open Q2). Node-RED fans a `split` into N independent messages; our engine
//! is one-shot-run (spine Decision 9 — no parked runs, no per-event fan-out storm), so we resolve Q2
//! to **array-carry**: `split` emits the sequence as a single envelope whose `payload` is the array
//! plus a top-level `parts` descriptor, and `join` reads `parts` to recombine. The array rides one
//! settle down the wire (the `parts` field carries forward like `topic`, D4), so split→transform→join
//! round-trips **without** a new frontier behaviour — split/join collapse to pure array transforms
//! (exactly the collapse the scope predicted). Per-element work between them is the `map` node.
//!
//! ## The `parts` contract (Risk 2 — designed once, reused by split/join/batch)
//!
//! `parts = { "id": <seq-id>, "count": <n>, "kind": "array"|"object", "keys"?: [<k>...] }`. `split`
//! stamps it; `join` consumes + strips it. For an object source `split` emits the array of **values**
//! in key order and records `keys` so `join` can rebuild the object. `count` is the element count.
//! The `id` groups a sequence (a caller-supplied `topic`-like tag, or the literal `"seq"`); it exists
//! so a future streaming `join` can key on it — array-carry doesn't need it, but the field is part of
//! the versioned contract from day one.

use serde_json::{Map, Value};

/// The envelope field name carrying the sequence descriptor (additive to the D6 envelope).
pub const PARTS: &str = "parts";

/// `split`: one array/object `payload` → the sequence envelope fields `{ payload: <array>, parts }`.
/// An array splits into its elements; an object into its values (key order), recording `keys` so
/// `join` can rebuild it. A scalar/null splits into a one-element sequence (Node-RED wraps a lone
/// message). Returns the emitted envelope map (`payload` + `parts`), not just a payload, because
/// `split` sets a second envelope field.
pub fn split(config: &Value, payload: &Value) -> Result<Map<String, Value>, String> {
    let seq_id = config
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("seq")
        .to_string();
    let (elements, kind, keys): (Vec<Value>, &str, Option<Vec<Value>>) = match payload {
        Value::Array(a) => (a.clone(), "array", None),
        Value::Object(m) => {
            let keys: Vec<Value> = m.keys().map(|k| Value::String(k.clone())).collect();
            let vals: Vec<Value> = m.values().cloned().collect();
            (vals, "object", Some(keys))
        }
        // A lone scalar/null → a one-element sequence (Node-RED wraps a single message).
        other => (vec![other.clone()], "array", None),
    };
    let mut parts = Map::new();
    parts.insert("id".into(), Value::String(seq_id));
    parts.insert("count".into(), Value::Number(elements.len().into()));
    parts.insert("kind".into(), Value::String(kind.to_string()));
    if let Some(keys) = keys {
        parts.insert("keys".into(), Value::Array(keys));
    }
    let mut out = Map::new();
    out.insert("payload".into(), Value::Array(elements));
    out.insert(PARTS.into(), Value::Object(parts));
    Ok(out)
}

/// `join`: recombine a `split` sequence back into an array/object, keyed by the incoming `parts`
/// (which carried forward down the wire, D4). With `parts.kind == "object"` and `parts.keys` present,
/// zips the (possibly transformed) array back into an object by those keys; otherwise returns the
/// array as-is (order preserved). Absent `parts` (a plain array reached `join` directly) → the array
/// unchanged (a no-op join is still well-defined). `join` emits **only** `payload`; the executor
/// drops the consumed `parts` because `join` does not re-emit it (a non-carried field).
///
/// `inputs` is the node's whole incoming message so `join` can read both `payload` and the
/// carried-forward `parts`.
pub fn join(inputs: &Map<String, Value>) -> Result<Value, String> {
    let payload = inputs.get("payload").cloned().unwrap_or(Value::Null);
    let arr = match &payload {
        Value::Array(a) => a.clone(),
        // A non-array payload joins to itself (nothing to recombine).
        other => return Ok(other.clone()),
    };
    let parts = inputs.get(PARTS);
    let kind = parts
        .and_then(|p| p.get("kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("array");
    if kind == "object" {
        let keys = parts
            .and_then(|p| p.get("keys"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut obj = Map::new();
        for (i, k) in keys.iter().enumerate() {
            let Some(key) = k.as_str() else { continue };
            let val = arr.get(i).cloned().unwrap_or(Value::Null);
            obj.insert(key.to_string(), val);
        }
        return Ok(Value::Object(obj));
    }
    Ok(Value::Array(arr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn split_array_stamps_parts() {
        let out = split(&json!({}), &json!([10, 20, 30])).unwrap();
        assert_eq!(out["payload"], json!([10, 20, 30]));
        assert_eq!(out["parts"]["count"], json!(3));
        assert_eq!(out["parts"]["kind"], json!("array"));
        assert_eq!(out["parts"]["id"], json!("seq"));
    }

    #[test]
    fn split_object_records_keys() {
        let out = split(&json!({"id": "s1"}), &json!({"a": 1, "b": 2})).unwrap();
        assert_eq!(out["payload"], json!([1, 2]));
        assert_eq!(out["parts"]["kind"], json!("object"));
        assert_eq!(out["parts"]["keys"], json!(["a", "b"]));
        assert_eq!(out["parts"]["id"], json!("s1"));
    }

    #[test]
    fn split_scalar_is_one_element_sequence() {
        let out = split(&json!({}), &json!(7)).unwrap();
        assert_eq!(out["payload"], json!([7]));
        assert_eq!(out["parts"]["count"], json!(1));
    }

    #[test]
    fn join_array_round_trips_preserving_order() {
        // split then join an array → identity (order preserved).
        let s = split(&json!({}), &json!([1, 2, 3])).unwrap();
        let mut inputs = Map::new();
        inputs.insert("payload".into(), s["payload"].clone());
        inputs.insert("parts".into(), s["parts"].clone());
        let joined = join(&inputs).unwrap();
        assert_eq!(joined, json!([1, 2, 3]));
    }

    #[test]
    fn join_rebuilds_object_from_keys() {
        let s = split(&json!({}), &json!({"a": 1, "b": 2})).unwrap();
        // simulate a per-element transform that doubled each value between split and join.
        let doubled: Vec<Value> = s["payload"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| json!(v.as_i64().unwrap() * 2))
            .collect();
        let mut inputs = Map::new();
        inputs.insert("payload".into(), json!(doubled));
        inputs.insert("parts".into(), s["parts"].clone());
        let joined = join(&inputs).unwrap();
        assert_eq!(joined, json!({"a": 2, "b": 4}));
    }

    #[test]
    fn join_without_parts_is_identity() {
        let mut inputs = Map::new();
        inputs.insert("payload".into(), json!([1, 2]));
        assert_eq!(join(&inputs).unwrap(), json!([1, 2]));
    }
}
