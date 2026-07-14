//! `grafana-map` — the Grafana import pin (grafana-parity-backend-scope, P3).
//!
//! One dep-light crate beside `lb-viz` (no host/store/bus dependency) that turns a raw Grafana
//! dashboard export into a normalized v1 JSON ready for the mapper: **detect** the shape (accept v1,
//! reject v2/snapshot with a pointer), apply the **ported v33 migration subset** (datasource-string→
//! ref, panel-type renames), then **resolve `__inputs`** against a caller-supplied value map and strip
//! the three import envelopes. Grafana JSON stays interchange throughout — never storage.
//!
//! Both consumers call [`pin`]: the standalone converter workspace (as a git dep, ending its
//! vendor-vs-path question for this module) and the `dashboard.import` verb (import-export-scope).
//! The verb owns datasource-uid remap and the final map to our `Dashboard`; this crate owns only the
//! interchange-normalization pin.

mod detect;
mod inputs;
mod migrate;

use serde_json::Value;
use std::collections::HashMap;

pub use detect::{detect, Shape};
pub use inputs::{resolve_inputs, InputReport};
pub use migrate::{migrate_v1, MigrateReport, PINNED_SCHEMA_VERSION};

/// Why an export could not be pinned.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PinError {
    /// A v2 (app-platform) dashboard — the mapper has no v2 path. Points at the supported shape.
    #[error(
        "Grafana v2 (app-platform) dashboards are not supported; export as the classic \
             (schemaVersion) model or convert first"
    )]
    UnsupportedV2,
    /// A snapshot export, not a plain dashboard.
    #[error("Grafana snapshot exports are not importable as dashboards")]
    Snapshot,
    /// The top-level JSON was not an object.
    #[error("export root is not a JSON object")]
    NotAnObject,
}

/// The combined report from a successful pin — everything the caller must surface so nothing is
/// silently dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinReport {
    /// What the migration subset did (and any version degradation notice).
    pub migrate: MigrateReport,
    /// `__inputs` resolution outcome (unresolved names, auto-fills, `__requires`).
    pub inputs: InputReport,
}

impl PinReport {
    /// True when the migration ran cleanly AND every `__inputs` entry resolved.
    pub fn is_clean(&self) -> bool {
        self.migrate.degraded.is_none() && self.inputs.is_fully_resolved()
    }
}

/// Pin a raw Grafana export into normalized v1 JSON, mutating `root` in place.
///
/// `input_values` maps each `__inputs` entry name to its replacement (typically the caller's
/// federation datasource uid). `__expr__` inputs auto-fill without an entry. Order matters: detect
/// first (reject v2/snapshot before touching anything), migrate the classic model, then resolve
/// `${NAME}` tokens against `__inputs` — so a `${DS_*}` sitting inside a freshly-wrapped
/// `{"uid": "${DS_*}"}` ref still resolves.
pub fn pin(
    root: &mut Value,
    input_values: &HashMap<String, String>,
) -> Result<PinReport, PinError> {
    if !root.is_object() {
        return Err(PinError::NotAnObject);
    }
    match detect(root) {
        Shape::V2 => Err(PinError::UnsupportedV2),
        Shape::Snapshot => Err(PinError::Snapshot),
        Shape::V1 { schema_version } => {
            let migrate = migrate_v1(root, schema_version);
            let inputs = resolve_inputs(root, input_values);
            Ok(PinReport { migrate, inputs })
        }
    }
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
    fn full_pin_migrates_then_resolves_inputs() {
        // A pre-v33 export: string datasource is a ${DS_*} token, graph panel, __inputs present.
        let mut root = json!({
            "schemaVersion": 30,
            "__inputs": [{"name": "DS_PROMETHEUS", "type": "datasource", "pluginId": "prometheus"}],
            "__requires": [{"type": "grafana", "id": "grafana", "version": "9.0.0"}],
            "panels": [{
                "type": "graph",
                "datasource": "${DS_PROMETHEUS}",
                "targets": [{"datasource": "${DS_PROMETHEUS}", "expr": "up"}]
            }]
        });
        let report = pin(&mut root, &vals(&[("DS_PROMETHEUS", "fed-prom")])).unwrap();

        // panel-type rename
        assert_eq!(root["panels"][0]["type"], json!("timeseries"));
        // datasource wrapped THEN the ${DS_*} inside the ref resolved
        assert_eq!(root["panels"][0]["datasource"], json!({"uid": "fed-prom"}));
        assert_eq!(
            root["panels"][0]["targets"][0]["datasource"],
            json!({"uid": "fed-prom"})
        );
        // envelopes stripped
        assert!(root.get("__inputs").is_none());
        assert!(root.get("__requires").is_none());
        // reports carried
        assert!(report.is_clean());
        assert_eq!(report.inputs.requires, vec!["grafana 9.0.0".to_string()]);
    }

    #[test]
    fn v2_rejected_with_pointer() {
        let mut root = json!({"apiVersion": "dashboard.grafana.app/v2beta1", "spec": {}});
        assert_eq!(
            pin(&mut root, &HashMap::new()),
            Err(PinError::UnsupportedV2)
        );
    }

    #[test]
    fn snapshot_rejected() {
        let mut root = json!({"snapshot": {"key": "x"}, "panels": []});
        assert_eq!(pin(&mut root, &HashMap::new()), Err(PinError::Snapshot));
    }

    #[test]
    fn unresolved_input_pin_is_not_clean_but_succeeds() {
        let mut root = json!({
            "schemaVersion": 33,
            "__inputs": [{"name": "DS_X", "type": "datasource", "pluginId": "prometheus"}],
            "panels": [{"type": "stat", "datasource": "${DS_X}"}]
        });
        let report = pin(&mut root, &HashMap::new()).unwrap();
        assert!(!report.is_clean());
        assert_eq!(report.inputs.unresolved, vec!["DS_X".to_string()]);
    }

    #[test]
    fn non_object_root_errors() {
        let mut root = json!([1, 2, 3]);
        assert_eq!(pin(&mut root, &HashMap::new()), Err(PinError::NotAnObject));
    }
}
