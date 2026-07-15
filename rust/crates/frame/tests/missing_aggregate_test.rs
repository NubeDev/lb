//! Missing-data + aggregate verbs on the seeded fixture frame (tests mirror `missing.rs` +
//! `aggregate.rs`). The fixture's one null `value` pins the "nulls skipped, and said so" policy.

mod support;

use serde_json::json;
use support::{engine, eval_err, eval_fixture};

#[test]
fn drop_nulls_variants() {
    let e = engine();
    assert_eq!(
        eval_fixture(&e, "f.drop_nulls().height()").unwrap(),
        json!(5)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.drop_nulls(["ts"]).height()"#).unwrap(),
        json!(6)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.drop_nulls(["value"]).height()"#).unwrap(),
        json!(5)
    );
}

#[test]
fn fill_null_replaces_everywhere() {
    let e = engine();
    assert_eq!(
        eval_fixture(&e, r#"f.fill_null(0.0).sort("value").records()[0].value"#).unwrap(),
        json!(0.0)
    );
}

#[test]
fn fill_null_strategies() {
    let e = engine();
    // Row 2 (a, ts 300) is the null; forward takes row 1's 7.0.
    assert_eq!(
        eval_fixture(
            &e,
            r#"f.fill_null_strategy("value", "forward").records()[2].value"#
        )
        .unwrap(),
        json!(7.0)
    );
    // Backward takes row 3's 5.0.
    assert_eq!(
        eval_fixture(
            &e,
            r#"f.fill_null_strategy("value", "backward").records()[2].value"#
        )
        .unwrap(),
        json!(5.0)
    );
    // Mean of the 5 non-null values = 5.0.
    assert_eq!(
        eval_fixture(
            &e,
            r#"f.fill_null_strategy("value", "mean").records()[2].value"#
        )
        .unwrap(),
        json!(5.0)
    );
    assert_eq!(
        eval_fixture(
            &e,
            r#"f.fill_null_strategy("value", "zero").records()[2].value"#
        )
        .unwrap(),
        json!(0.0)
    );
    let err = eval_err(
        &e,
        &format!(
            "{}\n{}",
            support::FIXTURE,
            r#"f.fill_null_strategy("value", "nope")"#
        ),
    );
    assert!(err.contains("forward|backward|mean|zero"), "got: {err}");
}

#[test]
fn scalar_aggregates_skip_nulls() {
    let e = engine();
    assert_eq!(eval_fixture(&e, r#"f.mean("value")"#).unwrap(), json!(5.0));
    assert_eq!(
        eval_fixture(&e, r#"f.median("value")"#).unwrap(),
        json!(5.0)
    );
    assert_eq!(eval_fixture(&e, r#"f.sum("value")"#).unwrap(), json!(25.0));
    assert_eq!(eval_fixture(&e, r#"f.min("value")"#).unwrap(), json!(1.0));
    assert_eq!(eval_fixture(&e, r#"f.max("value")"#).unwrap(), json!(9.0));
    // Sample stats over [3, 7, 5, 1, 9]: variance = 10, std = sqrt(10). (`var` is a rhai
    // reserved keyword, hence `variance`.)
    assert_eq!(
        eval_fixture(&e, r#"f.variance("value")"#).unwrap(),
        json!(10.0)
    );
    let std = eval_fixture(&e, r#"f.std("value")"#)
        .unwrap()
        .as_f64()
        .unwrap();
    assert!((std - 10.0f64.sqrt()).abs() < 1e-12, "std was {std}");
}

#[test]
fn quantile_interpolates_linearly() {
    let e = engine();
    // Sorted non-null values [1, 3, 5, 7, 9]: q 0.5 -> 5, q 0.25 -> 3.
    assert_eq!(
        eval_fixture(&e, r#"f.quantile("value", 0.5)"#).unwrap(),
        json!(5.0)
    );
    assert_eq!(
        eval_fixture(&e, r#"f.quantile("value", 0.25)"#).unwrap(),
        json!(3.0)
    );
    let err = eval_err(
        &e,
        &format!("{}\n{}", support::FIXTURE, r#"f.quantile("value", 1.5)"#),
    );
    assert!(err.contains("0.0..=1.0"), "got: {err}");
}

#[test]
fn count_and_n_unique() {
    let e = engine();
    assert_eq!(eval_fixture(&e, "f.count()").unwrap(), json!(6));
    assert_eq!(
        eval_fixture(&e, r#"f.n_unique("series")"#).unwrap(),
        json!(2)
    );
    assert_eq!(eval_fixture(&e, r#"f.n_unique("ts")"#).unwrap(), json!(3));
}

#[test]
fn value_counts_ranks_by_frequency() {
    let e = engine();
    let rows = eval_fixture(
        &e,
        r#"f.filter_not_null("value").value_counts("series").records()"#,
    )
    .unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0]["series"],
        json!("b"),
        "b has 3 non-null values, a has 2"
    );
    assert_eq!(rows[0]["count"], json!(3));
    assert_eq!(rows[1]["count"], json!(2));
}
