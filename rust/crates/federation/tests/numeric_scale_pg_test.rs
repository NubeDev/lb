//! Postgres `numeric` values survive the direct path EXACTLY, whatever order the rows arrive in
//! (fine-grained-data-path scope §Testing plan).
//!
//! The connector builds one Arrow `Decimal128` column per result and takes its scale from the FIRST
//! non-null row, then rescales every later row down to it — a lossy round. A result whose first row
//! happens to be a whole number therefore truncated the entire column: `20, 17.685, 15.334` came
//! back as `20.0, 18.0, 15.0`, and the SAME values reordered came back exact. That is the shape this
//! file pins, so the fixtures below are deliberately ordered **widest value last**: ordered the other
//! way every assertion here passes against the broken code and proves nothing.
//!
//! A single-row result is always exact (its own scale wins), which is why the defect survived casual
//! testing — so every fixture here is multi-row.
//!
//! Requires the `postgres` feature AND a reachable Postgres (`LB_TEST_PG_DSN`, else the repo's dev
//! container on :5433). Without one each test prints a SKIP line and returns green rather than
//! faking a pass (testing-scope §0).

#![cfg(feature = "postgres")]

#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/event.rs"]
mod event;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/info_schema.rs"]
mod info_schema;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/pool.rs"]
mod pool;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/query.rs"]
mod query;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/results.rs"]
mod results;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/source/mod.rs"]
mod source;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/validate.rs"]
mod validate;

use source::Source;
use validate::validate_select;

/// Same DSN convention as the sibling direct-path suite: `LB_TEST_PG_DSN`, else the dev container
/// the repo ships (`docker/`, listening on 5433) in the libpq KEY=VALUE form the pool parses. A test
/// credential against a local container — never a production secret.
fn test_dsn() -> String {
    std::env::var("LB_TEST_PG_DSN")
        .unwrap_or_else(|_| "host=localhost port=5433 user=lb password=lb_secret dbname=lb".into())
}

/// Connect, or `None` with a printed SKIP if Postgres is unreachable — an honest skip, never a
/// silent green over an unexercised path.
async fn try_connect() -> Option<std::sync::Arc<dyn Source>> {
    match source::connect("postgres", &test_dsn()).await {
        Ok(s) => match s.probe().await {
            Ok(()) => Some(s),
            Err(e) => {
                eprintln!("SKIP: Postgres probe failed ({e}); is the dev container up on :5433?");
                None
            }
        },
        Err(e) => {
            eprintln!("SKIP: Postgres connect failed ({e}); set LB_TEST_PG_DSN or start the dev container");
            None
        }
    }
}

/// Run `sql` through the DIRECT path — the exact path a `federation.query` (and thus a dashboard
/// panel) takes for any query that reads no synthetic `information_schema` view.
async fn run_direct(source: &dyn Source, sql: &str) -> query::QueryResult {
    let validated = validate_select(sql).expect("validate");
    assert!(
        validated.is_simple,
        "test query must take the direct path (no information_schema): {sql}"
    );
    query::run_via_direct_for_test(source, &validated, sql)
        .await
        .expect("direct run")
}

/// The single-column values of a one-column result, as f64 (every cell must be a number, never a
/// string or null — a `numeric` that arrived as text would also "not be rounded" and must not pass).
fn column_f64(out: &query::QueryResult) -> Vec<f64> {
    out.rows
        .iter()
        .map(|r| {
            let cell = &r.as_array().expect("row is an array")[0];
            cell.as_f64()
                .unwrap_or_else(|| panic!("cell is not a JSON number: {cell:?}"))
        })
        .collect()
}

/// Every value must survive to the last decimal it was stored with. Compared against an exact
/// literal with a tolerance far tighter than the rounding under test (which loses whole tenths).
fn assert_exact(got: &[f64], want: &[f64], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: row count");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert!(
            (g - w).abs() < 1e-9,
            "{what}: row {i} came back {g}, not {w} — the column was rounded to another row's scale"
        );
    }
}

/// The reported repro, as literals: an unconstrained `numeric` result whose FIRST row is a whole
/// number and whose later rows carry three decimals. Against the first-row-scale behaviour this
/// returns `20.0, 18.0, 15.0`; the same three values with the widest first are exact even then, so
/// the second half of this test is the control that proves the ordering is what does the damage.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_path_keeps_later_rows_exact_after_a_whole_number_first_row() {
    let Some(source) = try_connect().await else {
        return;
    };

    // Whole number FIRST — the ordering that loses data. Do not "tidy" this into ascending order.
    let out = run_direct(
        source.as_ref(),
        "SELECT v FROM (VALUES (20::numeric),(17.685::numeric),(15.334::numeric)) t(v)",
    )
    .await;
    assert_exact(
        &column_f64(&out),
        &[20.0, 17.685, 15.334],
        "whole-number first",
    );

    // Control: the identical values, widest first. This was ALWAYS exact — if it ever fails, the
    // fix broke ordinary decoding rather than the rounding.
    let out = run_direct(
        source.as_ref(),
        "SELECT v FROM (VALUES (17.685::numeric),(20::numeric),(15.334::numeric)) t(v)",
    )
    .await;
    assert_exact(&column_f64(&out), &[17.685, 20.0, 15.334], "widest first");
}

/// The estate-shaped case: a stored time series in an unconstrained `numeric` column, read newest
/// first, whose newest sample happens to be a whole number. This is how the defect reached a chart —
/// a 2 °C decline rendered as a two-value staircase, and the same series read from a window with a
/// fractional newest sample rendered correctly.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_path_keeps_a_stored_numeric_series_exact() {
    let Some(source) = try_connect().await else {
        return;
    };
    let table = format!("lb_numscale_{}", std::process::id());
    source
        .exec_raw_for_test(&format!("DROP TABLE IF EXISTS {table}"))
        .await
        .expect("drop");
    // `numeric` with NO declared scale — what a `histories.value` column and every `max()`/`avg()`
    // over one actually is. A `numeric(10,3)` would take the schema-driven path and never show this.
    source
        .exec_raw_for_test(&format!("CREATE TABLE {table} (ts int, v numeric)"))
        .await
        .expect("create");
    source
        .exec_raw_for_test(&format!(
            "INSERT INTO {table} VALUES (1, 20), (2, 19.82), (3, 19.64), (4, 19.47)"
        ))
        .await
        .expect("insert");

    // Newest first → the whole number leads. The scale of row 1 must not decide rows 2..n.
    let out = run_direct(
        source.as_ref(),
        &format!("SELECT v FROM {table} ORDER BY ts"),
    )
    .await;
    assert_exact(
        &column_f64(&out),
        &[20.0, 19.82, 19.64, 19.47],
        "stored series",
    );

    // An aggregate over the same column is an unconstrained `numeric` too, and is the common
    // exposure (a threshold compared after the round trip makes a wrong decision, not wrong text).
    let out = run_direct(
        source.as_ref(),
        &format!("SELECT max(v) AS m FROM {table} GROUP BY ts ORDER BY ts"),
    )
    .await;
    assert_exact(
        &column_f64(&out),
        &[20.0, 19.82, 19.64, 19.47],
        "grouped max",
    );

    let _ = source
        .exec_raw_for_test(&format!("DROP TABLE {table}"))
        .await;
}
