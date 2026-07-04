//! The Phase 0 f.sql security probe (scope Risk "f.sql is a wide surface"). The cage's
//! `f.sql("… FROM self")` (Phase 2) wraps a polars `SQLContext` with ONE table registered (`self`).
//! The scope's #1 f.sql risk demands: "assert polars' SQL context cannot reach registration of
//! external scans". That is a RUNTIME property (does `read_csv`/`read_parquet` actually execute
//! against a path/url), not just a feature-gate audit — so we probe it here, on the exact feature set
//! the crate compiles with. If any of these scans SUCCEED, the cage has an I/O leak and Phase 0 must
//! STOP (scope: "I'd rather rescope").
//!
//! Lives in `tests/` (not a unit test) because the probe is about polars-sql behavior against the
//! pinned feature set — it constructs its own `SQLContext`, the way the Phase 2 `f.sql` verb will.

use lb_frame::frame_from_json;
use polars::prelude::*;
use polars::sql::SQLContext;
use serde_json::{json, Value};

fn rows() -> Vec<Value> {
    vec![
        json!({ "ts": 1_i64, "value": 3.0_f64, "series": "a" }),
        json!({ "ts": 2_i64, "value": 7.0_f64, "series": "a" }),
        json!({ "ts": 3_i64, "value": 5.0_f64, "series": "b" }),
    ]
}

/// The cage's intended use: register exactly one table named `self`, run a SELECT against it.
fn self_frame() -> DataFrame {
    let df = frame_from_json(&rows()).expect("build frame");
    let mut ctx = SQLContext::new();
    ctx.register("self", df.clone().lazy());
    ctx.execute("SELECT series, avg(value) AS v FROM self GROUP BY series")
        .expect("self-scan should succeed")
        .collect()
        .expect("collect")
}

#[test]
fn sql_self_only_select_works() {
    let out = self_frame();
    assert_eq!(out.shape(), (2, 2));
    // GROUP BY order is not guaranteed — collect the series labels + values and compare as multisets.
    let mut labels = out
        .column("series")
        .unwrap()
        .as_materialized_series()
        .str()
        .unwrap()
        .iter()
        .map(|x| x.unwrap().to_string())
        .collect::<Vec<_>>();
    labels.sort();
    assert_eq!(labels, vec!["a".to_string(), "b".to_string()]);
    // a: (3.0+7.0)/2 = 5.0 ; b: 5.0
    let mut v = out
        .column("v")
        .unwrap()
        .as_materialized_series()
        .f64()
        .unwrap()
        .iter()
        .map(|x| x.unwrap())
        .collect::<Vec<_>>();
    v.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(v, vec![5.0, 5.0]);
}

#[test]
fn sql_cannot_reach_an_unregistered_table() {
    // A bare `FROM othertable` (no read_* function) is rejected: nothing else is registered.
    let df = frame_from_json(&rows()).unwrap();
    let mut ctx = SQLContext::new();
    ctx.register("self", df.lazy());
    let err = ctx.execute("SELECT * FROM othertable");
    assert!(
        err.is_err(),
        "unregistered table scan must be rejected: {}",
        err.err().unwrap()
    );
}

#[test]
fn sql_cannot_read_csv_from_disk() {
    // The decisive probe: does `read_csv('<path>')` actually open a file? We point it at /dev/null
    // so it cannot leak real data either way. See `assert_external_scan_rejected` for the criteria.
    assert_external_scan_rejected("SELECT * FROM read_csv('/dev/null')", "/dev/null");
}

#[test]
fn sql_cannot_read_parquet_from_disk() {
    // The same probe for parquet (the polars-parquet crate IS compiled transitively, so this must
    // be verified, not assumed).
    assert_external_scan_rejected("SELECT * FROM read_parquet('/etc/hostname')", "hostname");
}

/// The shared assertion: an external-scan SQL must be rejected WITHOUT reaching the path. `Ok` means
/// the scan ran (I/O leak); an error mentioning the path means it reached the FS (leak); any other
/// error (function-not-registered / parse) is the safe outcome.
fn assert_external_scan_rejected(sql: &str, leak_marker: &str) {
    let df = frame_from_json(&rows()).unwrap();
    let mut ctx = SQLContext::new();
    ctx.register("self", df.lazy());
    match ctx.execute(sql) {
        Ok(_) => panic!("SQL scan SUCCEEDED — polars-sql can reach I/O (leak): {sql}"),
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            assert!(
                !msg.contains(&leak_marker.to_lowercase())
                    && !msg.contains("no such file")
                    && !msg.contains("permission denied"),
                "SQL scan reached the filesystem ({sql}): {msg}"
            );
            eprintln!("  [safe] {sql} → rejected: {e}");
        }
    }
}
