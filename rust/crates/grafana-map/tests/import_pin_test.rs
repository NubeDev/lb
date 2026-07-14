//! Integration test: the import pin against **real Grafana export fixtures** (P3 testing plan —
//! no fakes; real exports are fixtures). Drives [`grafana_map::pin`] end-to-end and asserts the
//! interchange normalization: migration subset applied, `__inputs` resolved name-keyed, all three
//! envelopes stripped, v2 rejected with a pointer.

use grafana_map::{pin, PinError};
use serde_json::{json, Value};
use std::collections::HashMap;

fn load(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {path}: {e}"))
}

fn vals(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn prom_v30_export_pins_cleanly() {
    let mut root = load("prom_v30_export.json");
    let report = pin(
        &mut root,
        &vals(&[("DS_PROMETHEUS", "fed-prom-uid"), ("VAR_JOB", "node")]),
    )
    .expect("v1 export should pin");

    // Both ported migration steps ran; no version degradation (v30 is above the floor).
    assert_eq!(report.migrate.from_version, 30);
    assert!(report.migrate.degraded.is_none());
    assert!(report.is_clean(), "all inputs resolved");

    // panel-type renames: graph -> timeseries, singlestat -> stat, gauge-mode singlestat -> gauge.
    assert_eq!(root["panels"][0]["type"], json!("timeseries"));
    assert_eq!(root["panels"][1]["type"], json!("stat"));
    assert_eq!(root["panels"][2]["panels"][0]["type"], json!("gauge"));

    // datasource-string -> {uid}, with the ${DS_*} inside resolved to the federation uid.
    let want_ds = json!({ "uid": "fed-prom-uid" });
    assert_eq!(root["panels"][0]["datasource"], want_ds);
    assert_eq!(root["panels"][0]["targets"][0]["datasource"], want_ds);
    assert_eq!(root["templating"]["list"][0]["datasource"], want_ds);
    assert_eq!(root["panels"][2]["panels"][0]["datasource"], want_ds);

    // VAR_JOB constant substituted into the title (embedded token).
    assert_eq!(root["title"], json!("Node Exporter — node"));

    // All three envelopes stripped; __requires reported informationally first.
    assert!(root.get("__inputs").is_none());
    assert!(root.get("__requires").is_none());
    assert!(root.get("__elements").is_none());
    assert!(report
        .inputs
        .requires
        .iter()
        .any(|r| r.starts_with("grafana")));
}

#[test]
fn v2beta1_export_rejected_with_pointer() {
    let mut root = load("v2beta1_export.json");
    let err = pin(&mut root, &HashMap::new()).unwrap_err();
    assert_eq!(err, PinError::UnsupportedV2);
    // The error carries a pointer to the supported shape.
    assert!(err.to_string().contains("classic"));
}
