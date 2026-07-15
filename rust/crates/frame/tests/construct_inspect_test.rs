//! Construction + inspection verbs on the seeded fixture frame (tests mirror `construct.rs` +
//! `inspect.rs`).

mod support;

use serde_json::json;
use support::{engine, eval_err, eval_fixture, eval_json};

#[test]
fn frame_builds_from_records_and_reports_shape() {
    let e = engine();
    assert_eq!(eval_fixture(&e, "f.shape()").unwrap(), json!([6, 3]));
    assert_eq!(eval_fixture(&e, "f.height()").unwrap(), json!(6));
    assert_eq!(eval_fixture(&e, "f.width()").unwrap(), json!(3));
    assert_eq!(eval_fixture(&e, "f.is_empty()").unwrap(), json!(false));
}

#[test]
fn frame_rejects_non_map_elements() {
    let e = engine();
    let err = eval_err(&e, "frame([1, 2, 3])");
    assert!(err.contains("every element must be a map"), "got: {err}");
}

#[test]
fn empty_frame_is_empty() {
    let e = engine();
    assert_eq!(eval_json(&e, "frame([]).is_empty()").unwrap(), json!(true));
    assert_eq!(eval_json(&e, "frame([]).height()").unwrap(), json!(0));
}

#[test]
fn columns_and_dtypes_name_the_schema() {
    let e = engine();
    let cols = eval_fixture(&e, "f.columns()").unwrap();
    let names: Vec<&str> = cols
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(names.contains(&"ts") && names.contains(&"series") && names.contains(&"value"));
    let dtypes = eval_fixture(&e, "f.dtypes()").unwrap();
    assert_eq!(dtypes["ts"], json!("i64"));
    assert_eq!(dtypes["series"], json!("str"));
    assert_eq!(dtypes["value"], json!("f64"));
}

#[test]
fn head_tail_slice_bound_rows() {
    let e = engine();
    assert_eq!(eval_fixture(&e, "f.head(2).height()").unwrap(), json!(2));
    assert_eq!(
        eval_fixture(&e, "f.tail(1).records()[0].series").unwrap(),
        json!("b")
    );
    assert_eq!(
        eval_fixture(&e, "f.slice(1, 2).height()").unwrap(),
        json!(2)
    );
    assert_eq!(
        eval_fixture(&e, "f.slice(1, 2).records()[0].ts").unwrap(),
        json!(200)
    );
    // Over-asking clamps, never throws.
    assert_eq!(eval_fixture(&e, "f.head(99).height()").unwrap(), json!(6));
}

#[test]
fn null_count_sees_the_missing_value() {
    let e = engine();
    let nc = eval_fixture(&e, "f.null_count()").unwrap();
    assert_eq!(nc["value"], json!(1));
    assert_eq!(nc["ts"], json!(0));
}

#[test]
fn describe_summarizes_numeric_columns() {
    let e = engine();
    let rows = eval_fixture(&e, "f.describe().records()").unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 7, "count/null_count/mean/std/min/max/median");
    let stat = |name: &str| {
        rows.iter()
            .find(|r| r["statistic"] == json!(name))
            .unwrap_or_else(|| panic!("missing statistic {name}"))
            .clone()
    };
    assert_eq!(stat("count")["value"], json!(5.0));
    assert_eq!(stat("null_count")["value"], json!(1.0));
    assert_eq!(stat("mean")["value"], json!(5.0)); // (3+7+5+1+9)/5
    assert_eq!(stat("min")["value"], json!(1.0));
    assert_eq!(stat("max")["value"], json!(9.0));
    assert_eq!(stat("median")["value"], json!(5.0));
    assert_eq!(stat("min")["ts"], json!(100.0));
    // The string column is not summarized.
    assert!(stat("mean").get("series").is_none());
}
