//! The JSON ↔ polars `DataFrame` round-trip + column pluck (FILE-LAYOUT §4: tests mirror source).
//! Proves the pinned polars feature set builds + the catalog's `frame(records)`/`f.records()`/
//! `f.col("value")` shape is sound. NOT the catalog's per-verb tests (those land in Phase 2).

use lb_frame::{frame_col_json, frame_from_json, frame_to_json};
use serde_json::Value;

fn rows() -> Vec<Value> {
    vec![
        serde_json::json!({ "ts": 1_i64, "value": 3.0_f64, "series": "a" }),
        serde_json::json!({ "ts": 2_i64, "value": 7.0_f64, "series": "a" }),
        serde_json::json!({ "ts": 3_i64, "value": 5.0_f64, "series": "b" }),
    ]
}

#[test]
fn frame_round_trips_through_json() {
    let df = frame_from_json(&rows()).expect("build frame");
    assert_eq!(df.shape(), (3, 3));
    let out = frame_to_json(&df).expect("to json");
    assert_eq!(out.len(), 3);
    // shape preserved: ts is an integer, value a float, series a string
    assert_eq!(out[0]["ts"], 1_i64);
    assert_eq!(out[0]["value"], 3.0_f64);
    assert_eq!(out[0]["series"], "a");
}

#[test]
fn col_plucks_a_flat_array() {
    let df = frame_from_json(&rows()).expect("build frame");
    let col = frame_col_json(&df, "value").expect("pluck col");
    assert_eq!(col.len(), 3);
    assert_eq!(col[0], 3.0_f64);
    assert_eq!(col[1], 7.0_f64);
}

#[test]
fn empty_rows_give_empty_frame() {
    let df = frame_from_json(&[]).expect("empty frame");
    assert_eq!(df.shape(), (0, 0));
    assert_eq!(frame_to_json(&df).unwrap().len(), 0);
}
