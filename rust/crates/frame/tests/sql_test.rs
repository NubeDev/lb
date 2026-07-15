//! The `f.sql` verb through the rhai surface: happy path, syntax errors surface verbatim as
//! author feedback, and the self-only table registration (the deeper I/O probes live in
//! `sql_security_test.rs`, which drives the same `SQLContext` construction directly).

mod support;

use serde_json::json;
use support::{engine, eval_err, eval_fixture};

#[test]
fn sql_selects_and_aggregates_over_self() {
    let e = engine();
    let rows = eval_fixture(
        &e,
        r#"f.sql("SELECT series, AVG(value) AS v FROM self GROUP BY series ORDER BY series").records()"#,
    )
    .unwrap();
    assert_eq!(
        rows,
        json!([{ "series": "a", "v": 5.0 }, { "series": "b", "v": 5.0 }])
    );
}

#[test]
fn sql_syntax_error_surfaces_verbatim() {
    let e = engine();
    let err = eval_err(
        &e,
        &format!(
            "{}\n{}",
            support::FIXTURE,
            r#"f.sql("SELEC oops FROM self")"#
        ),
    );
    assert!(
        err.to_lowercase().contains("sql") || err.to_lowercase().contains("parse"),
        "the polars sql error must reach the author: {err}"
    );
}

#[test]
fn sql_cannot_see_other_tables() {
    let e = engine();
    let err = eval_err(
        &e,
        &format!(
            "{}\n{}",
            support::FIXTURE,
            r#"f.sql("SELECT * FROM other")"#
        ),
    );
    assert!(
        !err.is_empty(),
        "unregistered table must be rejected: {err}"
    );
}

#[test]
fn sql_cannot_read_files() {
    let e = engine();
    let err = eval_err(
        &e,
        &format!(
            "{}\n{}",
            support::FIXTURE,
            r#"f.sql("SELECT * FROM read_csv('/etc/hostname')")"#
        ),
    );
    let lower = err.to_lowercase();
    assert!(
        !lower.contains("hostname") && !lower.contains("no such file"),
        "the scan must be rejected before touching the filesystem: {err}"
    );
}
