//! Datasource collection + remap (viz import-export scope, the tenancy-critical step). A Grafana JSON
//! references datasources by `{type, uid}`; we **cannot auto-trust** those uids (a Grafana uid means
//! nothing here, and matching by name could bind across the workspace wall). So we:
//!
//! 1. **collect** every referenced `(type, uid)` from panels + targets + template variables, and
//! 2. on commit, **apply** the caller's chosen remap — replacing each `{uid}` with our resolved
//!    datasource, but ONLY after verifying the target is a datasource in the CALLER's workspace that
//!    the caller holds a grant for. The workspace wall + cap check are enforced here, server-side,
//!    never by the JSON.
//!
//! Anything left unmapped marks its panels "unmapped" (degraded, honest empty at render) — never faked.

use std::collections::BTreeSet;

use serde_json::Value;

use super::{DatasourceRemap, DegradedItem};

/// Walk a (migrated) Grafana dashboard and collect every distinct referenced datasource. After the P3
/// `__inputs` resolution + datasource-ref migration, each datasource is a `{type?, uid}` object; a
/// null/absent datasource ("default") and the special expression uid are skipped (nothing to remap).
pub fn collect(root: &Value) -> Vec<DatasourceRemap> {
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut out = Vec::new();
    walk(root, &mut |ds| {
        let uid = ds.get("uid").and_then(Value::as_str).unwrap_or("");
        if uid.is_empty() || uid == "__expr__" {
            return; // the default datasource / server-side expression — nothing to bind.
        }
        let kind = ds
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if seen.insert((kind.clone(), uid.to_string())) {
            out.push(DatasourceRemap {
                kind,
                uid: uid.to_string(),
                mapped_to: String::new(),
            });
        }
    });
    out
}

/// Invoke `f` on every `datasource` object found anywhere in the tree (panel/target/variable level).
fn walk(v: &Value, f: &mut impl FnMut(&Value)) {
    match v {
        Value::Object(map) => {
            if let Some(ds) = map.get("datasource") {
                if ds.is_object() {
                    f(ds);
                }
            }
            for val in map.values() {
                walk(val, f);
            }
        }
        Value::Array(arr) => arr.iter().for_each(|e| walk(e, f)),
        _ => {}
    }
}

/// Rewrite every `datasource` ref in the tree by the caller's `mappings`, in place. A ref whose uid has
/// a non-empty `mapped_to` is rewritten to `{ "uid": <mapped_to> }` (our workspace datasource name);
/// an unmapped ref is left as-is and its owning panels degrade. Returns the degraded list for the
/// unmapped uids so the report can name them. Grant/workspace verification of each `mapped_to` happens
/// in the verb BEFORE this is called — this file only performs the substitution.
pub fn apply(root: &mut Value, mappings: &[DatasourceRemap]) -> Vec<DegradedItem> {
    let mut degraded = Vec::new();
    walk_mut(root, &mut |ds| {
        let Some(uid) = ds.get("uid").and_then(Value::as_str) else {
            return;
        };
        if uid.is_empty() || uid == "__expr__" {
            return;
        }
        match mappings.iter().find(|m| m.uid == uid) {
            Some(m) if !m.mapped_to.is_empty() => {
                *ds = serde_json::json!({ "uid": m.mapped_to });
            }
            _ => {
                degraded.push(DegradedItem {
                    kind: "datasource".to_string(),
                    cell: String::new(),
                    detail: format!("datasource '{uid}' not mapped — panels using it render empty"),
                });
            }
        }
    });
    // Dedup the degraded notices (one per uid, not one per occurrence).
    degraded.sort_by(|a, b| a.detail.cmp(&b.detail));
    degraded.dedup_by(|a, b| a.detail == b.detail);
    degraded
}

fn walk_mut(v: &mut Value, f: &mut impl FnMut(&mut Value)) {
    match v {
        Value::Object(map) => {
            if let Some(ds) = map.get_mut("datasource") {
                if ds.is_object() {
                    f(ds);
                }
            }
            for val in map.values_mut() {
                walk_mut(val, f);
            }
        }
        Value::Array(arr) => arr.iter_mut().for_each(|e| walk_mut(e, f)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn collects_distinct_datasources_skipping_default_and_expr() {
        let root = json!({
            "panels": [
                {"datasource": {"type": "prometheus", "uid": "prom-a"},
                 "targets": [{"datasource": {"type": "prometheus", "uid": "prom-a"}}]},
                {"datasource": {"type": "mysql", "uid": "db-b"}},
                {"datasource": {"uid": "__expr__"}},
                {"datasource": null}
            ]
        });
        let found = collect(&root);
        assert_eq!(found.len(), 2);
        assert!(found
            .iter()
            .any(|d| d.uid == "prom-a" && d.kind == "prometheus"));
        assert!(found.iter().any(|d| d.uid == "db-b"));
    }

    #[test]
    fn apply_rewrites_mapped_and_degrades_unmapped() {
        let mut root = json!({
            "panels": [
                {"datasource": {"type": "prometheus", "uid": "prom-a"}},
                {"datasource": {"type": "mysql", "uid": "db-b"}}
            ]
        });
        let mappings = vec![DatasourceRemap {
            kind: "prometheus".into(),
            uid: "prom-a".into(),
            mapped_to: "our-metrics".into(),
        }];
        let degraded = apply(&mut root, &mappings);
        assert_eq!(
            root["panels"][0]["datasource"],
            json!({"uid": "our-metrics"})
        );
        // db-b unmapped → left as-is + degraded once.
        assert_eq!(
            root["panels"][1]["datasource"],
            json!({"type": "mysql", "uid": "db-b"})
        );
        assert_eq!(degraded.len(), 1);
        assert!(degraded[0].detail.contains("db-b"));
    }
}
