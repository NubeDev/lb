//! Group/combine + series + time-bucket verbs on the seeded fixture frame (tests mirror
//! `group.rs`, `series.rs`, `timebucket.rs`).

mod support;

use serde_json::json;
use support::{engine, eval_err, eval_fixture, eval_json};

#[test]
fn group_agg_keeps_stable_key_order_and_names() {
    let e = engine();
    let rows = eval_fixture(
        &e,
        r#"f.drop_nulls().group_agg(["series"], #{ value: "mean", ts: "max" }).records()"#,
    )
    .unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["series"], json!("a"), "first-seen key order");
    assert_eq!(rows[0]["value"], json!(5.0)); // (3+7)/2
    assert_eq!(rows[0]["ts"], json!(200));
    assert_eq!(rows[1]["series"], json!("b"));
    assert_eq!(rows[1]["value"], json!(5.0)); // (5+1+9)/3
    let err = eval_err(
        &e,
        &format!(
            "{}\n{}",
            support::FIXTURE,
            r#"f.group_agg(["series"], #{ value: "nope" })"#
        ),
    );
    assert!(err.contains("unknown aggregation"), "got: {err}");
}

#[test]
fn join_inner_left_outer_anti() {
    let e = engine();
    let setup = r#"
        let left = frame([#{ k: "a", x: 1 }, #{ k: "b", x: 2 }, #{ k: "c", x: 3 }]);
        let right = frame([#{ k: "a", y: 10 }, #{ k: "b", y: 20 }, #{ k: "d", y: 40 }]);
    "#;
    assert_eq!(
        eval_json(
            &e,
            &format!("{setup} left.join(right, \"k\", \"inner\").height()")
        )
        .unwrap(),
        json!(2)
    );
    assert_eq!(
        eval_json(
            &e,
            &format!("{setup} left.join(right, \"k\", \"left\").height()")
        )
        .unwrap(),
        json!(3)
    );
    assert_eq!(
        eval_json(
            &e,
            &format!("{setup} left.join(right, \"k\", \"outer\").height()")
        )
        .unwrap(),
        json!(4)
    );
    let anti = eval_json(
        &e,
        &format!("{setup} left.join(right, \"k\", \"anti\").sort(\"k\").records()"),
    )
    .unwrap();
    assert_eq!(anti, json!([{ "k": "c", "x": 3 }]));
    let err = eval_err(&e, &format!("{setup} left.join(right, \"k\", \"cross\")"));
    assert!(err.contains("inner|left|outer|anti"), "got: {err}");
}

#[test]
fn vstack_appends_rows() {
    let e = engine();
    assert_eq!(eval_fixture(&e, "f.vstack(f).height()").unwrap(), json!(12));
}

#[test]
fn pivot_widens_deterministically() {
    let e = engine();
    let rows = eval_fixture(
        &e,
        r#"f.drop_nulls().pivot("ts", "series", "value", "mean").sort("ts").records()"#,
    )
    .unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 3, "one row per ts");
    assert_eq!(rows[0]["ts"], json!(100));
    assert_eq!(rows[0]["a"], json!(3.0));
    assert_eq!(rows[0]["b"], json!(5.0));
    assert_eq!(
        rows[2]["a"],
        json!(null),
        "a has no non-null value at ts 300"
    );
    assert_eq!(rows[2]["b"], json!(9.0));
}

#[test]
fn melt_lengthens() {
    let e = engine();
    let rows = eval_fixture(&e, r#"f.melt(["ts", "series"], ["value"]).records()"#).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0]["variable"], json!("value"));
    assert_eq!(rows[0]["value"], json!(3.0));
    assert!(rows[0].get("ts").is_some() && rows[0].get("series").is_some());
}

#[test]
fn rolling_windows_lead_with_nulls() {
    let e = engine();
    let sorted = r#"let g = f.drop_nulls().sort("value");"#; // values 1,3,5,7,9
    let mean = eval_fixture(
        &e,
        &format!("{sorted} g.rolling_mean(\"value\", 2).col(\"value_rolling_mean\")"),
    )
    .unwrap();
    assert_eq!(mean, json!([null, 2.0, 4.0, 6.0, 8.0]));
    let sum = eval_fixture(
        &e,
        &format!("{sorted} g.rolling_sum(\"value\", 2).col(\"value_rolling_sum\")"),
    )
    .unwrap();
    assert_eq!(sum, json!([null, 4.0, 8.0, 12.0, 16.0]));
    let min = eval_fixture(
        &e,
        &format!("{sorted} g.rolling_min(\"value\", 2).col(\"value_rolling_min\")"),
    )
    .unwrap();
    assert_eq!(min, json!([null, 1.0, 3.0, 5.0, 7.0]));
    let max = eval_fixture(
        &e,
        &format!("{sorted} g.rolling_max(\"value\", 2).col(\"value_rolling_max\")"),
    )
    .unwrap();
    assert_eq!(max, json!([null, 3.0, 5.0, 7.0, 9.0]));
    let std = eval_fixture(
        &e,
        &format!("{sorted} g.rolling_std(\"value\", 2).col(\"value_rolling_std\")"),
    )
    .unwrap();
    let s = std.as_array().unwrap();
    assert_eq!(s[0], json!(null));
    assert!((s[1].as_f64().unwrap() - 2.0f64.sqrt()).abs() < 1e-12);
}

#[test]
fn series_derivations() {
    let e = engine();
    let sorted = r#"let g = f.drop_nulls().sort("value");"#; // 1,3,5,7,9
    assert_eq!(
        eval_fixture(
            &e,
            &format!("{sorted} g.diff(\"value\").col(\"value_diff\")")
        )
        .unwrap(),
        json!([null, 2.0, 2.0, 2.0, 2.0])
    );
    assert_eq!(
        eval_fixture(
            &e,
            &format!("{sorted} g.cumsum(\"value\").col(\"value_cumsum\")")
        )
        .unwrap(),
        json!([1.0, 4.0, 9.0, 16.0, 25.0])
    );
    assert_eq!(
        eval_fixture(
            &e,
            &format!("{sorted} g.shift(\"value\", 1).col(\"value_shift\")")
        )
        .unwrap(),
        json!([null, 1.0, 3.0, 5.0, 7.0])
    );
    let pct = eval_fixture(
        &e,
        &format!("{sorted} g.pct_change(\"value\").col(\"value_pct_change\")"),
    )
    .unwrap();
    let pct = pct.as_array().unwrap();
    assert_eq!(pct[0], json!(null));
    assert!((pct[1].as_f64().unwrap() - 2.0).abs() < 1e-12); // 1 -> 3
    assert_eq!(
        eval_fixture(
            &e,
            &format!("{sorted} g.rank(\"value\").col(\"value_rank\")")
        )
        .unwrap(),
        json!([1.0, 2.0, 3.0, 4.0, 5.0])
    );
    let ewm = eval_fixture(
        &e,
        &format!("{sorted} g.ewm_mean(\"value\", 0.5).col(\"value_ewm_mean\")"),
    )
    .unwrap();
    let ewm = ewm.as_array().unwrap();
    assert_eq!(ewm[0], json!(1.0));
    assert_eq!(ewm[1], json!(2.0)); // 0.5*3 + 0.5*1
    let z = eval_fixture(
        &e,
        &format!("{sorted} g.zscore(\"value\").col(\"value_zscore\")"),
    )
    .unwrap();
    let z = z.as_array().unwrap();
    assert!((z[2].as_f64().unwrap()).abs() < 1e-12, "the mean scores 0");
    assert_eq!(
        eval_fixture(
            &e,
            r#"f.clip("value", 3.0, 7.0).drop_nulls().sort("value").col("value")"#
        )
        .unwrap(),
        json!([3.0, 3.0, 5.0, 7.0, 7.0])
    );
}

#[test]
fn bucket_truncates_secs_and_ms() {
    let e = engine();
    // 15m = 900s buckets; secs epochs.
    let rows = eval_json(
        &e,
        r#"frame([#{ ts: 1000 }, #{ ts: 1799 }, #{ ts: 1800 }]).bucket("ts", "15m").col("ts")"#,
    )
    .unwrap();
    assert_eq!(rows, json!([900, 900, 1800]));
    // Millisecond epochs bucket by 900_000.
    let ms = eval_json(
        &e,
        r#"frame([#{ ts: 1700000123456 }]).bucket("ts", "15m").col("ts")"#,
    )
    .unwrap();
    assert_eq!(ms, json!([1700000100000_i64]));
    let err = eval_err(&e, r#"frame([#{ ts: 1 }]).bucket("ts", "15x")"#);
    assert!(err.contains("s|m|h|d|w"), "got: {err}");
}
