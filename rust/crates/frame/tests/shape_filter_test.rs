//! Shape + filter verbs on the seeded fixture frame (tests mirror `shape.rs` + `filter.rs`).

mod support;

use serde_json::json;
use support::{engine, eval_err, eval_fixture};

#[test]
fn select_and_drop_project_columns() {
    let e = engine();
    assert_eq!(
        eval_fixture(&e, r#"f.select(["ts", "value"]).columns()"#).unwrap(),
        json!(["ts", "value"])
    );
    assert_eq!(
        eval_fixture(&e, r#"f.drop(["series"]).width()"#).unwrap(),
        json!(2)
    );
    let err = eval_err(
        &e,
        &format!("{}\n{}", support::FIXTURE, r#"f.select(["nope"])"#),
    );
    assert!(err.contains("nope"), "unknown column named in error: {err}");
}

#[test]
fn rename_keeps_data() {
    let e = engine();
    assert_eq!(
        eval_fixture(&e, r#"f.rename("value", "temp").records()[0].temp"#).unwrap(),
        json!(3.0)
    );
}

#[test]
fn with_col_from_adds_a_column_of_matching_length() {
    let e = engine();
    assert_eq!(
        eval_fixture(
            &e,
            r#"f.with_col_from("flag", [1, 2, 3, 4, 5, 6]).records()[5].flag"#
        )
        .unwrap(),
        json!(6)
    );
    let err = eval_err(
        &e,
        &format!(
            "{}\n{}",
            support::FIXTURE,
            r#"f.with_col_from("flag", [1, 2])"#
        ),
    );
    assert!(!err.is_empty(), "length mismatch must error: {err}");
}

#[test]
fn sort_orders_rows_nulls_last() {
    let e = engine();
    assert_eq!(
        eval_fixture(&e, r#"f.sort("value").records()[0].value"#).unwrap(),
        json!(1.0)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.sort("value").records()[5].value"#).unwrap(),
        json!(null)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.sort("value", true).records()[0].value"#).unwrap(),
        json!(9.0)
    );
}

#[test]
fn unique_and_unique_by_dedupe() {
    let e = engine();
    assert_eq!(
        eval_fixture(&e, r#"f.vstack(f).unique().height()"#).unwrap(),
        json!(6)
    );
    // Two series -> unique_by keeps the first row of each.
    assert_eq!(
        eval_fixture(&e, r#"f.unique_by(["series"]).height()"#).unwrap(),
        json!(2)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.unique_by(["series"]).records()[0].value"#).unwrap(),
        json!(3.0)
    );
}

#[test]
fn reverse_flips_row_order() {
    let e = engine();
    assert_eq!(
        eval_fixture(&e, r#"f.reverse().records()[0].value"#).unwrap(),
        json!(9.0)
    );
}

#[test]
fn comparison_filters_keep_matching_rows() {
    let e = engine();
    assert_eq!(
        eval_fixture(&e, r#"f.filter_eq("series", "a").height()"#).unwrap(),
        json!(3)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.filter_ne("series", "a").height()"#).unwrap(),
        json!(3)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.filter_gt("value", 5.0).height()"#).unwrap(),
        json!(2)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.filter_ge("value", 5.0).height()"#).unwrap(),
        json!(3)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.filter_lt("value", 3.0).height()"#).unwrap(),
        json!(1)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.filter_le("value", 3.0).height()"#).unwrap(),
        json!(2)
    );
    // Int literal against an i64 column.
    assert_eq!(
        eval_fixture(&e, r#"f.filter_eq("ts", 100).height()"#).unwrap(),
        json!(2)
    );
}

#[test]
fn filter_in_between_and_null_filters() {
    let e = engine();
    assert_eq!(
        eval_fixture(&e, r#"f.filter_in("value", [3.0, 9.0]).height()"#).unwrap(),
        json!(2)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.filter_between("value", 3.0, 7.0).height()"#).unwrap(),
        json!(3)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.filter_null("value").height()"#).unwrap(),
        json!(1)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.filter_not_null("value").height()"#).unwrap(),
        json!(5)
    );
}

#[test]
fn sample_is_deterministic_for_a_seed() {
    let e = engine();
    let a = eval_fixture(&e, r#"f.sample(3, 42).records()"#).unwrap();
    let b = eval_fixture(&e, r#"f.sample(3, 42).records()"#).unwrap();
    assert_eq!(a, b, "same seed, same rows");
    assert_eq!(a.as_array().unwrap().len(), 3);
    let c = eval_fixture(&e, r#"f.sample(3, 7).records()"#).unwrap();
    assert_ne!(a, c, "different seed, different rows (with 6C3 room)");
    // n > height clamps.
    assert_eq!(
        eval_fixture(&e, r#"f.sample(99, 1).height()"#).unwrap(),
        json!(6)
    );
}
