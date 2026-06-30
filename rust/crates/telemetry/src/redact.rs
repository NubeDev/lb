//! `params_digest` — record tool params as a **digest + a redacted shape summary**, never the raw
//! value (observability scope, README §6.7; shared with audit). The digest is SHA-256 of a
//! canonical JSON encoding; the shape summary is the value's type tree with sizes, so a reader can
//! tell "object with 3 string fields, one ~12 chars" without seeing the contents. Low-entropy params
//! (a single short string) are summarized as a **shape only** — no bare hash a reader could brute
//! force back to the value (audit scope risk).
//!
//! This is the structural guarantee the planted-value redaction test (telemetry-console scope) leans
//! on: a tool param carrying the secret reaches a telemetry event ONLY as `params_digest`, so the
//! secret string appears in zero stored rows.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Produce the digest + shape summary of `params`: a compact string `<digest>:<shape>`. The digest
/// is a hex SHA-256 over canonical (sorted-key) JSON; the shape is the type tree. A caller passes
/// the raw params here and STORES the result — never the raw value.
pub fn params_digest(params: &Value) -> String {
    let canonical = canonical_json(params);
    let digest = hex(Sha256::digest(canonical.as_bytes()).as_slice());
    format!("{}:{}", digest, shape_of(params))
}

/// Canonical JSON: stable key order + compact, so the digest is reproducible across machines.
fn canonical_json(v: &Value) -> String {
    match v {
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .into_iter()
                .map(|k| {
                    format!(
                        "{}:{}",
                        canonical_json(&Value::String(k.clone())),
                        canonical_json(&o[k])
                    )
                })
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(a) => {
            let inner: Vec<String> = a.iter().map(canonical_json).collect();
            format!("[{}]", inner.join(","))
        }
        Value::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        other => other.to_string(),
    }
}

/// The type tree of a value, with sizes — the redacted shape a reader sees instead of contents.
/// A scalar is its type name; a string is `str{len}`; an object is `{k:shape,...}`; an array is
/// `[shape,...]`.
fn shape_of(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(_) => "bool".into(),
        Value::Number(_) => "num".into(),
        Value::String(s) => format!("str{}", s.len()),
        Value::Array(a) => {
            let inner: Vec<String> = a.iter().map(shape_of).collect();
            format!("[{}]", inner.join(","))
        }
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .into_iter()
                .map(|k| format!("{}:{}", k, shape_of(&o[k])))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_is_reproducible_regardless_of_key_order() {
        let a = serde_json::json!({ "b": 1, "a": "x" });
        let b = serde_json::json!({ "a": "x", "b": 1 });
        assert_eq!(params_digest(&a), params_digest(&b), "canonical key order");
    }

    #[test]
    fn shape_redacts_the_value() {
        let d = params_digest(&serde_json::json!({ "token": "super-secret-value-123" }));
        assert!(
            !d.contains("super-secret-value-123"),
            "the raw value must NOT appear in the digest/shape"
        );
        assert!(d.contains("str22"), "shape records the string length only");
    }

    #[test]
    fn different_values_digest_differently() {
        let a = params_digest(&serde_json::json!({ "x": "one" }));
        let b = params_digest(&serde_json::json!({ "x": "two" }));
        assert_ne!(a, b);
    }
}
