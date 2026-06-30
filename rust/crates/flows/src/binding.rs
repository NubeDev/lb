//! Resolve a node's `with` bindings against recorded upstream outputs + flow params (the chain
//! binding grammar verbatim — rule-chains-scope, lifted into JSON). A binding is exactly one
//! **whole-value** reference or a literal, never a partial-interpolation string:
//! - `${steps.<id>.output}` — the upstream node's output value;
//! - `${steps.<id>.findings}` — the upstream node's findings;
//! - `${params.<name>}` — a flow/subflow parameter (Decision 4);
//! - or a literal (a JSON scalar/object/array).
//!
//! `no templating mini-language` — an embedded `${` inside a reference is rejected. A failed/skipped
//! upstream resolves to `null` (under `Continue`); a missing param resolves to `null`. The resolver
//! is pure over a `recorded` map + `params`, so it is exercised identically by the run engine and
//! the editor's wire inspector.

use serde_json::{Map, Value};
use std::collections::HashMap;

/// A recorded upstream node output: the output value + the findings value (a node's recorded result
/// downstream `${steps.x.output}` / `${steps.x.findings}` read).
#[derive(Debug, Clone, Default)]
pub struct NodeOutput {
    pub output: Value,
    pub findings: Value,
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

enum Reference<'a> {
    Param(&'a str),
    StepOutput(&'a str),
    StepFindings(&'a str),
}

fn lookup(
    reference: Reference<'_>,
    recorded: &HashMap<String, NodeOutput>,
    params: &Map<String, Value>,
) -> Value {
    match reference {
        Reference::Param(name) => params.get(name).cloned().unwrap_or(Value::Null),
        Reference::StepOutput(id) => recorded
            .get(id)
            .map(|r| r.output.clone())
            .unwrap_or(Value::Null),
        Reference::StepFindings(id) => recorded
            .get(id)
            .map(|r| r.findings.clone())
            .unwrap_or(Value::Null),
    }
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
    let (id, field) = rest.rsplit_once('.')?;
    match field {
        "output" => Some(Reference::StepOutput(id)),
        "findings" => Some(Reference::StepFindings(id)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

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
    fn resolves_step_output_reference() {
        let mut rec = HashMap::new();
        rec.insert(
            "a".to_string(),
            NodeOutput {
                output: json!({"v": 1}),
                findings: json!([1]),
            },
        );
        let v = resolve_value(&json!("${steps.a.output}"), &rec, &Map::new()).unwrap();
        assert_eq!(v, json!({"v": 1}));
        let v = resolve_value(&json!("${steps.a.findings}"), &rec, &Map::new()).unwrap();
        assert_eq!(v, json!([1]));
    }

    #[test]
    fn missing_upstream_resolves_null() {
        let v = resolve_value(
            &json!("${steps.unknown.output}"),
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
    fn rejects_partial_interpolation() {
        // a partial-interpolation string is a literal, NOT a reference (no templating mini-language).
        let v = resolve_value(
            &json!("prefix-${steps.a.output}"),
            &HashMap::new(),
            &Map::new(),
        )
        .unwrap();
        assert_eq!(v, json!("prefix-${steps.a.output}"));
    }
}
