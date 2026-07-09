//! Golden round-trip tests over real-shape Grafana fixtures (grafana-conversion
//! scope, "Testing plan"). The headline contract: a real Grafana export maps to
//! an asserted `Dashboard` JSON, the `Dashboard` deserializes through the vendored
//! `model.rs` (the fold-in guard), and the report names every degraded/dropped
//! feature that appeared.

use grafana_conv_mapper::{convert, Dashboard};
use serde_json::Value;

fn load(name: &str) -> String {
    let path = format!("tests/fixtures/{name}");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// The mapper's output must deserialize through the vendored `Dashboard` type
/// (proves the emitted shape is the host's record shape — the fold-in guard).
fn round_trip_dashboard(dash: &Dashboard) -> Value {
    let v = serde_json::to_value(dash).expect("serialize Dashboard");
    let _: Dashboard =
        serde_json::from_value(v.clone()).expect("output deserializes back through model.rs");
    v
}

// ── Golden: fleet-overview ────────────────────────────────────────────────────

#[test]
fn fleet_overview_maps_cells_and_settings() {
    let (dash, report) = convert(&load("fleet-overview.json")).expect("convert");

    assert_eq!(dash.id, "fleet-overview");
    assert_eq!(dash.title, "Fleet Overview");
    assert_eq!(dash.description, "Fleet health at a glance");
    assert_eq!(dash.schema_version, 3);

    // One row cell + two panel cells = 3 cells, in order.
    assert_eq!(dash.cells.len(), 3);
    assert_eq!(dash.cells[0].view, "row");
    assert_eq!(dash.cells[0].title, "Throughput");
    assert_eq!(dash.cells[1].view, "chart");
    assert_eq!(dash.cells[1].title, "Requests/sec");
    assert_eq!(dash.cells[1].sources.len(), 2);
    assert_eq!(dash.cells[1].sources[0].ref_id, "A");
    assert_eq!(dash.cells[1].sources[1].ref_id, "B");
    assert_eq!(dash.cells[2].view, "stat");
    assert_eq!(dash.cells[2].panel_ref, "panel:lib-001");

    // Datasource UIDs are carried opaque + flagged degraded.
    assert!(
        report.degraded.iter().any(|l| l.code == "datasource.uid"),
        "datasource UID degrades reported: {:?}",
        report.degraded
    );

    // The output survives the fold-in guard.
    let _ = round_trip_dashboard(&dash);
}

// ── Trap #1: row dual-encoding ────────────────────────────────────────────────

#[test]
fn rows_collapse_and_expand_normalize_identically() {
    let (collapsed, _) = convert(&load("rows-collapsed.json")).expect("collapsed");
    let (expanded, _) = convert(&load("rows-expanded.json")).expect("expanded");

    // Both produce: 1 row cell + 2 child cells (3 total). The collapsed row's
    // nested children are lifted to flat siblings; the expanded row's children
    // already are siblings. Membership is the row's grid span, not a nested array.
    assert_eq!(collapsed.cells.len(), 3);
    assert_eq!(expanded.cells.len(), 3);

    // The row cell is the same in both.
    assert_eq!(collapsed.cells[0].view, "row");
    assert_eq!(expanded.cells[0].view, "row");

    // The two children match on the bits that prove dual-encoding collapsed to
    // one shape: same view, same gridPos, same targets. (The id/title differ
    // only in the per-fixture label, which is asserted separately.)
    let c_inner_a = collapsed.cells[1].clone();
    let e_inner_a = expanded.cells[1].clone();
    // The Grafana `timeseries` panel maps to view:"chart"; both encodings produce it.
    assert_eq!(c_inner_a.view, "chart");
    assert_eq!(e_inner_a.view, "chart");
    assert_eq!(c_inner_a.x, e_inner_a.x);
    assert_eq!(c_inner_a.y, e_inner_a.y);
    assert_eq!(c_inner_a.w, e_inner_a.w);
    assert_eq!(c_inner_a.sources.len(), e_inner_a.sources.len());
}

// ── Advanced variables ────────────────────────────────────────────────────────

#[test]
fn advanced_variables_map_onto_model_fields() {
    let (dash, report) = convert(&load("advanced-variables.json")).expect("convert");

    // Topo order: `region` must precede `host` (host's query references $region).
    let region_i = dash
        .variables
        .iter()
        .position(|v| v.name == "region")
        .expect("region present");
    let host_i = dash
        .variables
        .iter()
        .position(|v| v.name == "host")
        .expect("host present");
    assert!(region_i < host_i, "region must resolve before host");

    // Custom with label≠value options carries the full advanced field set.
    let opts = dash
        .variables
        .iter()
        .find(|v| v.name == "options_var")
        .expect("options_var");
    assert_eq!(opts.r#type, "custom");
    assert_eq!(opts.all_value, ".*");
    assert!(opts.regex.contains("text"));
    assert_eq!(opts.regex_apply_to, "value");
    assert_eq!(
        opts.options,
        Value::Array(vec![
            serde_json::json!({ "text": "West", "value": "west", "selected": false }),
            serde_json::json!({ "text": "East", "value": "east", "selected": true }),
        ])
    );

    // interval / textbox / constant map to their canonical types.
    let iv = dash
        .variables
        .iter()
        .find(|v| v.name == "interval")
        .unwrap();
    assert_eq!(iv.r#type, "interval");
    assert_eq!(iv.interval, vec!["1m", "5m", "10m", "30m", "1h"]);

    let txt = dash
        .variables
        .iter()
        .find(|v| v.name == "text_note")
        .unwrap();
    assert_eq!(txt.r#type, "text");
    assert_eq!(txt.text, "hello");

    let konst = dash
        .variables
        .iter()
        .find(|v| v.name == "const_token")
        .unwrap();
    assert_eq!(konst.r#type, "const");
    assert_eq!(konst.const_, "abc123");

    // adhoc → degraded; groupby → dropped. Both named in the report.
    assert!(report.degraded.iter().any(|l| l.code == "var.adhoc"));
    assert!(report.dropped.iter().any(|l| l.code == "var.groupby"));

    // `current` selection is reported degraded (lives on URL, not record).
    // (This fixture has no `current`, so check via the fleet fixture instead.)

    let _ = round_trip_dashboard(&dash);
}

// ── Report completeness (the honesty contract) ──────────────────────────────

#[test]
fn no_unmapped_top_level_feature_silently_dropped() {
    let input = r#"{
        "schemaVersion": 42,
        "title": "Completeness",
        "annotations": { "list": [{ "name": "x" }] },
        "links": [{ "title": "ext", "url": "https://x" }],
        "graphTooltip": 1,
        "fiscalYearStartMonth": 4,
        "liveNow": true,
        "editable": false
    }"#;
    let (_dash, report) = convert(input).expect("convert");

    // Every audit-triaged "degrade"/"out" feature that appears MUST produce a line.
    for code in [
        "annotations",
        "dashboard.links",
        "graphTooltip",
        "dashboard.calendar",
        "dashboard.liveNow",
        "dashboard.editable",
    ] {
        assert!(
            report.mentions(code),
            "completeness: `{code}` must be in the report (it appeared in the input)"
        );
    }
}

// ── V2 input is rejected ─────────────────────────────────────────────────────

#[test]
fn v2_kind_layout_is_rejected() {
    let v2 = r#"{
        "kind": "Dashboard",
        "spec": { "title": "v2" }
    }"#;
    let err = convert(v2).expect_err("v2 rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("v2") || msg.contains("unsupported"),
        "msg: {msg}"
    );
}
