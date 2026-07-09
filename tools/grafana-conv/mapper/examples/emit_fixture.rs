// One-shot: print the real mapper output for the sample input, for the UI test fixture.
fn main() {
    let input = r#"{
  "schemaVersion": 42,
  "title": "Sample",
  "panels": [
    { "id": 1, "type": "timeseries", "gridPos": { "x": 0, "y": 0, "w": 12, "h": 8 }, "targets": [ { "refId": "A", "datasource": { "uid": "prom-001" } } ] }
  ]
}"#;
    let (dash, report) = grafana_conv_mapper::convert(input).unwrap();
    let dash = serde_json::to_value(&dash).unwrap();
    let out = serde_json::json!({ "dashboard": dash, "report": report });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
