//! Resolve a node's `with` bindings against recorded upstream **envelopes** + flow params (the
//! Node-RED message-envelope grammar — flow-message-envelope-scope D5). A binding is exactly one
//! **whole-value** reference or a literal, never a partial-interpolation string:
//! - `${steps.<id>}` — the upstream node's **whole envelope** (`{payload, topic, ...}`);
//! - `${steps.<id>.<dot.path>}` — a field path **into** that envelope (`payload`, `topic`,
//!   `findings`, `payload.items`, …) via a JSON-pointer-style walk (missing → `null`);
//! - `${params.<name>}` — a flow/subflow parameter (unchanged);
//! - or a literal (a JSON scalar/object/array).
//!
//! `no templating mini-language` — an embedded `${` inside a reference is rejected. There is no
//! special-cased `.output`/`.findings` form any more (D5): they are ordinary field paths into the
//! envelope (`.payload`, `.findings`). A failed/skipped upstream resolves to `null` (under
//! `Continue`); a missing param or missing field path resolves to `null`. The resolver is pure over a
//! `recorded` map + `params`, so it is exercised identically by the run engine and the editor.

use serde_json::{Map, Value};
use std::collections::HashMap;

/// A recorded upstream node output: the whole **envelope** (a JSON object with a `payload` slot and
/// optional `topic`/free metadata) that a downstream `${steps.x}` / `${steps.x.<path>}` reads.
#[derive(Debug, Clone, Default)]
pub struct NodeOutput {
    /// The node's recorded output envelope (D1/D9). Field paths walk into this.
    pub envelope: Value,
}

impl NodeOutput {
    pub fn new(envelope: Value) -> Self {
        Self { envelope }
    }
}

/// Resolve a node's `with` bindings into a JSON object keyed by the binding name. Each value is the
/// resolved reference (or the literal).
pub fn resolve_bindings(
    with: &Map<String, Value>,
    recorded: &HashMap<String, NodeOutput>,
    params: &Map<String, Value>,
) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    for (key, value) in with {
        out.insert(key.clone(), resolve_value(value, recorded, params)?);
    }
    Ok(out)
}

/// Resolve one binding value: a whole-string `${...}` reference, else a literal.
pub fn resolve_value(
    value: &Value,
    recorded: &HashMap<String, NodeOutput>,
    params: &Map<String, Value>,
) -> Result<Value, String> {
    let Value::String(s) = value else {
        return Ok(value.clone());
    };
    let Some(reference) = parse_reference(s) else {
        return Ok(value.clone());
    };
    Ok(lookup(reference, recorded, params))
}

/// The upstream node id a binding value references, when it is a whole-string `${steps.<id>...}`
/// reference — `None` for literals and `${params.*}`. The save-time cross-branch lint reads this
/// (flow-plain-wiring-scope): a `${steps.X}` where X can never be in the bound node's firing
/// lineage is a data-drop mistake, flagged at save instead of silently binding null per firing.
pub fn referenced_step(value: &Value) -> Option<&str> {
    let Value::String(s) = value else { return None };
    match parse_reference(s)? {
        Reference::Step(id) | Reference::StepPath(id, _) => Some(id),
        Reference::Param(_) => None,
    }
}

enum Reference<'a> {
    Param(&'a str),
    /// The upstream node's whole envelope.
    Step(&'a str),
    /// A dot-path field into the upstream node's envelope.
    StepPath(&'a str, &'a str),
}

fn lookup(
    reference: Reference<'_>,
    recorded: &HashMap<String, NodeOutput>,
    params: &Map<String, Value>,
) -> Value {
    match reference {
        Reference::Param(name) => params.get(name).cloned().unwrap_or(Value::Null),
        Reference::Step(id) => recorded
            .get(id)
            .map(|r| r.envelope.clone())
            .unwrap_or(Value::Null),
        Reference::StepPath(id, path) => recorded
            .get(id)
            .map(|r| walk_path(&r.envelope, path))
            .unwrap_or(Value::Null),
    }
}

/// Walk a dot-separated field path into a JSON value (`payload.items.0` style). A missing key, or an
/// index into a non-array/non-object, resolves to `null`. Array indices are numeric path segments.
fn walk_path(root: &Value, path: &str) -> Value {
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

/// Parse a whole-string `${...}` reference. Rejects embedded `${`/`}` (only whole references resolve).
fn parse_reference(s: &str) -> Option<Reference<'_>> {
    let inner = s.strip_prefix("${")?.strip_suffix('}')?;
    if inner.contains("${") || inner.contains('}') {
        return None;
    }
    let inner = inner.trim();
    if let Some(name) = inner.strip_prefix("params.") {
        return Some(Reference::Param(name));
    }
    let rest = inner.strip_prefix("steps.")?;
    // `${steps.<id>}` → whole envelope; `${steps.<id>.<path>}` → a field path into it. The id is the
    // first segment; everything after the first dot is the dot-path.
    match rest.split_once('.') {
        Some((id, path)) if !id.is_empty() && !path.is_empty() => {
            Some(Reference::StepPath(id, path))
        }
        _ => {
            if rest.is_empty() {
                None
            } else {
                Some(Reference::Step(rest))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn recorded_with(id: &str, envelope: Value) -> HashMap<String, NodeOutput> {
        let mut rec = HashMap::new();
        rec.insert(id.to_string(), NodeOutput::new(envelope));
        rec
    }

    #[test]
    fn literal_passes_through() {
        let m = Map::new();
        let rec = HashMap::new();
        let v = resolve_value(&json!(42), &rec, &m).unwrap();
        assert_eq!(v, json!(42));
        let v = resolve_value(&json!({"a": 1}), &rec, &m).unwrap();
        assert_eq!(v, json!({"a": 1}));
    }

    #[test]
    fn whole_envelope_reference() {
        let rec = recorded_with("a", json!({"payload": {"v": 1}, "topic": "t"}));
        let v = resolve_value(&json!("${steps.a}"), &rec, &Map::new()).unwrap();
        assert_eq!(v, json!({"payload": {"v": 1}, "topic": "t"}));
    }

    #[test]
    fn field_paths_into_the_envelope() {
        let rec = recorded_with(
            "a",
            json!({"payload": {"items": [10, 20]}, "topic": "kfc.temp", "findings": [1]}),
        );
        let payload = resolve_value(&json!("${steps.a.payload}"), &rec, &Map::new()).unwrap();
        assert_eq!(payload, json!({"items": [10, 20]}));
        let topic = resolve_value(&json!("${steps.a.topic}"), &rec, &Map::new()).unwrap();
        assert_eq!(topic, json!("kfc.temp"));
        let findings = resolve_value(&json!("${steps.a.findings}"), &rec, &Map::new()).unwrap();
        assert_eq!(findings, json!([1]));
        let nested = resolve_value(&json!("${steps.a.payload.items}"), &rec, &Map::new()).unwrap();
        assert_eq!(nested, json!([10, 20]));
        let indexed =
            resolve_value(&json!("${steps.a.payload.items.1}"), &rec, &Map::new()).unwrap();
        assert_eq!(indexed, json!(20));
    }

    #[test]
    fn missing_path_resolves_null() {
        let rec = recorded_with("a", json!({"payload": 1}));
        // a missing field path → null (incl. the old `.output`, now just an absent field)
        let v = resolve_value(&json!("${steps.a.nope}"), &rec, &Map::new()).unwrap();
        assert_eq!(v, Value::Null);
        let v = resolve_value(&json!("${steps.a.output}"), &rec, &Map::new()).unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn missing_upstream_resolves_null() {
        let v = resolve_value(&json!("${steps.unknown}"), &HashMap::new(), &Map::new()).unwrap();
        assert_eq!(v, Value::Null);
        let v = resolve_value(
            &json!("${steps.unknown.payload}"),
            &HashMap::new(),
            &Map::new(),
        )
        .unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn resolves_param_reference() {
        let mut params = Map::new();
        params.insert("x".into(), json!("hello"));
        let v = resolve_value(&json!("${params.x}"), &HashMap::new(), &params).unwrap();
        assert_eq!(v, json!("hello"));
    }

    #[test]
    fn referenced_step_extracts_the_step_id_only_for_step_references() {
        assert_eq!(referenced_step(&json!("${steps.a}")), Some("a"));
        assert_eq!(referenced_step(&json!("${steps.a.payload.x}")), Some("a"));
        // params, literals, partial interpolations, and non-strings are not step references.
        assert_eq!(referenced_step(&json!("${params.x}")), None);
        assert_eq!(referenced_step(&json!("plain")), None);
        assert_eq!(referenced_step(&json!("pre-${steps.a}")), None);
        assert_eq!(referenced_step(&json!(42)), None);
    }

    #[test]
    fn rejects_partial_interpolation() {
        // a partial-interpolation string is a literal, NOT a reference (no templating mini-language).
        let v = resolve_value(
            &json!("prefix-${steps.a.payload}"),
            &HashMap::new(),
            &Map::new(),
        )
        .unwrap();
        assert_eq!(v, json!("prefix-${steps.a.payload}"));
    }
}
