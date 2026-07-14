//! The `__inputs` resolver — Grafana's `dash_template_evaluator.go`, ported.
//!
//! A shared export carries a `__inputs` array (`[{name, type, pluginId, ...}]`) and sprinkles
//! `${NAME}` tokens through the tree (datasource uids, constants). Resolution is a **name-keyed
//! lookup**: for each input, the caller supplies a value (typically the target federation datasource
//! uid); we substitute every `${NAME}` occurrence with it. There is NO `DS_`/`VAR_` prefix magic —
//! the token is exactly the input's `name`. `pluginId == "__expr__"` datasource inputs auto-fill to
//! the expression uid without a caller value (Grafana does the same). Unresolved inputs are an
//! honest per-entry error, never a silent blank.
//!
//! Grafana's backend strips only `__inputs`; per the P3 scope we strip all three (`__inputs`,
//! `__requires`, `__elements`) from the stored record — `__requires` is reported-informational and
//! `__elements` (library panels) are handled upstream by the mapper. This file owns `__inputs`.

use serde_json::{Map, Value};
use std::collections::HashMap;

/// The magic pluginId whose datasource inputs auto-fill without a caller value (server-side expressions).
const EXPR_PLUGIN_ID: &str = "__expr__";
/// The uid Grafana uses for the built-in expression datasource.
const EXPR_UID: &str = "__expr__";

/// Outcome of resolving `__inputs` against a caller-supplied value map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputReport {
    /// `__inputs` names that had no caller value and were not auto-fillable — the import must fail
    /// or degrade per-entry on these.
    pub unresolved: Vec<String>,
    /// `__inputs` names that auto-filled (`__expr__`) — informational.
    pub auto_filled: Vec<String>,
    /// `__requires` entries, reported-informational (plugin/grafana version requirements).
    pub requires: Vec<String>,
}

impl InputReport {
    pub fn is_fully_resolved(&self) -> bool {
        self.unresolved.is_empty()
    }
}

/// Resolve `${NAME}` tokens in `root` from the export's own `__inputs` array plus a caller-supplied
/// `values` map (input name → replacement string, usually a federation datasource uid), then strip
/// the three import envelopes. Returns the report; `root` is mutated in place.
///
/// Substitution is textual over string leaves — a value `"${DS_PROM}"` or an embedded
/// `"${DS_PROM}-suffix"` both get the token replaced, matching Grafana's `${var}` interpolation.
pub fn resolve_inputs(root: &mut Value, values: &HashMap<String, String>) -> InputReport {
    let mut report = InputReport::default();

    // Build the name -> replacement map from the export's __inputs, honoring caller values and the
    // __expr__ auto-fill. An input with neither is unresolved.
    let mut subs: HashMap<String, String> = HashMap::new();
    if let Some(inputs) = root.get("__inputs").and_then(Value::as_array) {
        for input in inputs {
            let Some(name) = input.get("name").and_then(Value::as_str) else {
                continue;
            };
            let plugin_id = input.get("pluginId").and_then(Value::as_str);
            if let Some(v) = values.get(name) {
                subs.insert(name.to_string(), v.clone());
            } else if plugin_id == Some(EXPR_PLUGIN_ID) {
                subs.insert(name.to_string(), EXPR_UID.to_string());
                report.auto_filled.push(name.to_string());
            } else {
                report.unresolved.push(name.to_string());
            }
        }
    }

    // Collect __requires for the informational report before stripping.
    if let Some(reqs) = root.get("__requires").and_then(Value::as_array) {
        for r in reqs {
            let id = r.get("id").and_then(Value::as_str).unwrap_or("?");
            let ver = r.get("version").and_then(Value::as_str).unwrap_or("");
            report
                .requires
                .push(format!("{id} {ver}").trim().to_string());
        }
    }

    // Substitute across the whole tree.
    substitute(root, &subs);

    // Strip the three envelopes from the stored record (our deliberate delta from Grafana's backend).
    if let Value::Object(map) = root {
        map.remove("__inputs");
        map.remove("__requires");
        map.remove("__elements");
    }

    report
}

/// Replace every `${NAME}` token found in `subs` throughout the JSON tree's string leaves.
fn substitute(v: &mut Value, subs: &HashMap<String, String>) {
    match v {
        Value::String(s) => {
            if s.contains("${") {
                *s = replace_tokens(s, subs);
            }
        }
        Value::Array(arr) => arr.iter_mut().for_each(|e| substitute(e, subs)),
        Value::Object(map) => substitute_object(map, subs),
        _ => {}
    }
}

fn substitute_object(map: &mut Map<String, Value>, subs: &HashMap<String, String>) {
    for val in map.values_mut() {
        substitute(val, subs);
    }
}

/// Replace all `${NAME}` occurrences in a single string. Unknown tokens are left verbatim (a later
/// template variable may own them — the pin never blanks an unresolved `${...}`).
fn replace_tokens(s: &str, subs: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            if let Some(close) = s[i + 2..].find('}') {
                let name = &s[i + 2..i + 2 + close];
                if let Some(rep) = subs.get(name) {
                    out.push_str(rep);
                    i = i + 2 + close + 1;
                    continue;
                }
            }
        }
        // Not a known token start — copy this char.
        let ch_len = s[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        out.push_str(&s[i..i + ch_len]);
        i += ch_len;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn vals(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn name_keyed_substitution_no_prefix_magic() {
        let mut root = json!({
            "__inputs": [{"name": "DS_PROMETHEUS", "type": "datasource", "pluginId": "prometheus"}],
            "panels": [{"datasource": "${DS_PROMETHEUS}"}]
        });
        let report = resolve_inputs(&mut root, &vals(&[("DS_PROMETHEUS", "fed-prom-uid")]));
        assert!(report.is_fully_resolved());
        assert_eq!(root["panels"][0]["datasource"], json!("fed-prom-uid"));
        // envelope stripped
        assert!(root.get("__inputs").is_none());
    }

    #[test]
    fn expr_plugin_auto_fills_without_caller_value() {
        let mut root = json!({
            "__inputs": [{"name": "DS_EXPR", "type": "datasource", "pluginId": "__expr__"}],
            "panels": [{"datasource": "${DS_EXPR}"}]
        });
        let report = resolve_inputs(&mut root, &HashMap::new());
        assert!(report.is_fully_resolved());
        assert_eq!(report.auto_filled, vec!["DS_EXPR".to_string()]);
        assert_eq!(root["panels"][0]["datasource"], json!("__expr__"));
    }

    #[test]
    fn unresolved_input_is_reported_not_blanked() {
        let mut root = json!({
            "__inputs": [{"name": "DS_MISSING", "type": "datasource", "pluginId": "prometheus"}],
            "panels": [{"datasource": "${DS_MISSING}"}]
        });
        let report = resolve_inputs(&mut root, &HashMap::new());
        assert_eq!(report.unresolved, vec!["DS_MISSING".to_string()]);
        // token left verbatim — never blanked
        assert_eq!(root["panels"][0]["datasource"], json!("${DS_MISSING}"));
    }

    #[test]
    fn embedded_token_and_requires_reported() {
        let mut root = json!({
            "__inputs": [{"name": "VAR_HOST", "type": "constant"}],
            "__requires": [{"type": "grafana", "id": "grafana", "version": "9.0.0"}],
            "title": "load on ${VAR_HOST}:9090"
        });
        let report = resolve_inputs(&mut root, &vals(&[("VAR_HOST", "web01")]));
        assert_eq!(root["title"], json!("load on web01:9090"));
        assert_eq!(report.requires, vec!["grafana 9.0.0".to_string()]);
        assert!(root.get("__requires").is_none());
    }

    #[test]
    fn all_three_envelopes_stripped() {
        let mut root = json!({
            "__inputs": [], "__requires": [], "__elements": {"lib1": {}},
            "panels": []
        });
        resolve_inputs(&mut root, &HashMap::new());
        assert!(root.get("__inputs").is_none());
        assert!(root.get("__requires").is_none());
        assert!(root.get("__elements").is_none());
    }
}
