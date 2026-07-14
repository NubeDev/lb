//! v33 datasource migration — a **string** datasource becomes a `{type, uid}` ref.
//!
//! Pre-v33 Grafana stored `"datasource": "Prometheus"` (a name or a `${DS_*}` token). v33 moved to
//! a structured `{type, uid}` reference everywhere a datasource appears: panel-level, target-level,
//! and template-variable-level. The pin applies only the **structural** half of this rule — wrap the
//! string in `{"uid": <string>}` — because the name→type lookup is Grafana's live datasource list,
//! which the pin has no access to. The mapper (or `dashboard.import` verb) fills `type` from the
//! caller's federation datasource when it resolves the uid. A null datasource (the "default" marker)
//! and an already-structured ref are left untouched.
//!
//! Special uids (`-- Mixed --`, `-- Dashboard --`, `__expr__`) are strings too, so they wrap the same
//! way — the mapper degrades them per-target with a report line (import-export-scope owns that).

use serde_json::{json, Value};

/// Walk the dashboard tree and convert every string `datasource` field to `{uid: <string>}`.
/// Recurses through panels, nested panels (rows), targets, and templating variables uniformly —
/// any object key named `datasource` whose value is a bare string is rewritten.
pub fn migrate(root: &mut Value) {
    convert(root);
}

fn convert(v: &mut Value) {
    match v {
        Value::Object(map) => {
            if let Some(ds) = map.get_mut("datasource") {
                if let Value::String(s) = ds {
                    *ds = json!({ "uid": s });
                }
            }
            for val in map.values_mut() {
                convert(val);
            }
        }
        Value::Array(arr) => arr.iter_mut().for_each(convert),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_and_target_string_datasource_wrapped() {
        let mut root = json!({
            "panels": [{
                "datasource": "Prometheus",
                "targets": [{"datasource": "Prometheus", "expr": "up"}]
            }]
        });
        migrate(&mut root);
        assert_eq!(
            root["panels"][0]["datasource"],
            json!({"uid": "Prometheus"})
        );
        assert_eq!(
            root["panels"][0]["targets"][0]["datasource"],
            json!({"uid": "Prometheus"})
        );
    }

    #[test]
    fn already_structured_ref_untouched() {
        let mut root = json!({"panels": [{"datasource": {"type": "prometheus", "uid": "abc"}}]});
        migrate(&mut root);
        assert_eq!(
            root["panels"][0]["datasource"],
            json!({"type": "prometheus", "uid": "abc"})
        );
    }

    #[test]
    fn null_default_datasource_untouched() {
        let mut root = json!({"panels": [{"datasource": null}]});
        migrate(&mut root);
        assert_eq!(root["panels"][0]["datasource"], Value::Null);
    }

    #[test]
    fn special_mixed_uid_wraps_for_mapper_to_degrade() {
        let mut root = json!({"panels": [{"datasource": "-- Mixed --"}]});
        migrate(&mut root);
        assert_eq!(
            root["panels"][0]["datasource"],
            json!({"uid": "-- Mixed --"})
        );
    }
}
