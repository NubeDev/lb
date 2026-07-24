//! Direct-path correctness against a REAL Postgres (fine-grained-data-path scope §Testing plan).
//!
//! PR #100 added a direct query path that bypasses DataFusion for any query not touching the
//! synthetic `information_schema` views. Two things about that path could silently corrupt a
//! dashboard panel; both are pinned here against a real seeded Postgres table (no mock — the
//! `Source` trait is the one sanctioned fake-boundary, testing-scope §0, and this drives the REAL
//! Postgres engine behind it):
//!
//!   1. **Type preservation (the data-loss regression).** The direct path's Postgres cell converter
//!      (`cell_to_value`) once ended in `_ => Null`, so any Arrow type without an explicit arm —
//!      `jsonb`, `uuid`, arrays, `bytea`, … — vanished to `null`, invisible data loss in a panel.
//!      `direct_path_preserves_wide_types` seeds those types and asserts the DIRECT path returns their
//!      real values (concrete expected values, not a DataFusion comparison — the DataFusion per-table
//!      provider REFUSES a table containing `jsonb`, so it is not a valid oracle for these types).
//!      `direct_path_matches_datafusion_for_common_types` covers the types BOTH paths carry
//!      (int/float/text/bool/timestamptz) and asserts they agree cell for cell. Test 1 FAILS against
//!      the old `_ => Null`; the common-types parity test also caught a timestamptz `+00:00` vs `Z`
//!      rendering divergence the direct path introduced (now aligned to the DataFusion wire form).
//!
//!   2. **Remote LIMIT (the pushdown regression).** The direct path once did
//!      `truncate(ROW_CAP)` client-side — the source computed and shipped the whole result, then we
//!      threw the tail away. `direct_path_pushes_limit_into_source` proves the source now RECEIVES a
//!      bounded query: it seeds ROW_CAP + N rows and asserts exactly ROW_CAP come back, AND checks
//!      (via `EXPLAIN`) that the plan Postgres executed carries a `Limit` node — i.e. the bound was
//!      pushed into the engine, not applied after the fact.
//!
//! These require the `postgres` feature AND a reachable Postgres. The DSN comes from
//! `LB_TEST_PG_DSN`, falling back to the repo's dev TimescaleDB container. If no Postgres answers,
//! each test prints a SKIP line and returns green — it does not fake a pass (testing-scope §0: a
//! skip is loud, never a silent green over an unbuilt path).

#![cfg(feature = "postgres")]

#[path = "../src/event.rs"]
mod event;
#[path = "../src/info_schema.rs"]
mod info_schema;
#[path = "../src/pool.rs"]
mod pool;
#[path = "../src/query.rs"]
mod query;
#[path = "../src/results.rs"]
mod results;
#[path = "../src/source/mod.rs"]
mod source;
#[path = "../src/validate.rs"]
mod validate;

use source::Source;
use validate::{validate_select, ROW_CAP};

/// The DSN for the test Postgres. `LB_TEST_PG_DSN` overrides; default is the dev container the repo
/// ships (`docker/`), which listens on 5433. The DSN is a test credential against a local container —
/// never a production secret.
fn test_dsn() -> String {
    // libpq KEY=VALUE form (space-separated), NOT a `postgresql://` URL: the underlying
    // `PostgresConnectionPool` parses its `connection_string` by splitting on whitespace into
    // `key=value` pairs (`parse_connection_string`) — it does not understand a URL, which would
    // collapse to an empty config and fail with "invalid configuration". The dev container listens on
    // 5433 (`docker/`). Test credential against a local container — never a production secret.
    std::env::var("LB_TEST_PG_DSN").unwrap_or_else(|_| {
        "host=localhost port=5433 user=lb password=lb_secret dbname=lb".to_string()
    })
}

/// Connect, or `None` with a printed SKIP if Postgres is unreachable — so a machine without the dev
/// container reports honestly instead of failing or (worse) faking a green.
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

/// Run `sql` through the DIRECT path (the `is_simple` branch of `register_and_run`) and return its
/// `{columns, rows}` — the exact path a normal dashboard panel query takes.
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

/// Run the SAME `sql` through the DataFusion path (per-table providers + unparse/pushdown) — the
/// oracle the direct path must match, cell for cell.
async fn run_datafusion(source: &dyn Source, sql: &str) -> query::QueryResult {
    query::run_via_datafusion_for_test(source, sql)
        .await
        .expect("datafusion run")
}

/// Seed a table whose columns span the types the old `_ => Null` catch-all dropped. Unique name per
/// run (pid) so parallel workers don't collide. Returns the table name.
async fn seed_wide_types(source: &dyn Source) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    // Unique per call (pid + counter) so parallel tests seeding this shape don't collide on the same
    // table name (`CREATE` races / one test's DROP pulling the rug from another).
    let table = format!(
        "lb_wide_{}_{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    // DDL + INSERT go through the source's real write connection (apply_ddl is allow-listed DDL; the
    // seed rows use write_rows' sibling raw path here via apply_ddl for CREATE, then an INSERT DDL is
    // not allowed — so we seed values inside the CREATE-adjacent raw exec). We use apply_ddl for the
    // schema and a direct INSERT via the pool for the row (seed = real write path, testing-scope §3.1).
    // Types chosen because the connector's Postgres→Arrow layer CAN read them (so they reach
    // `cell_to_value`), yet they land on Arrow types with NO explicit arm in that fn — so the old
    // `_ => Null` dropped them. (Deliberately excluded: `numeric(40,0)` and `inet`, which the Arrow
    // layer itself refuses to deserialize — those hard-error on BOTH paths, a different failure mode
    // than the silent-null this test pins. `numeric(20,4)` fits Decimal128 and is kept.)
    let create = format!(
        "CREATE TABLE IF NOT EXISTS {table} (\
           c_int int, c_float float8, c_text text, c_bool bool, \
           c_jsonb jsonb, c_uuid uuid, c_num numeric(20,4), \
           c_arr int[], c_bytea bytea, c_ts timestamptz)"
    );
    let insert = format!(
        "INSERT INTO {table} VALUES \
           (7, 1.5, 'hello', true, '{{\"k\":42}}'::jsonb, \
            '11111111-2222-3333-4444-555555555555'::uuid, \
            3.1416, '{{1,2,3}}'::int[], '\\xdeadbeef'::bytea, \
            '2026-01-02T03:04:05Z'::timestamptz), \
           (NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL)"
    );
    // Both statements are internal test SQL (no caller input), run through the source's real pool via
    // the raw exec seam the source exposes for tests.
    source
        .exec_raw_for_test(&format!("DROP TABLE IF EXISTS {table}"))
        .await
        .expect("drop old");
    source.exec_raw_for_test(&create).await.expect("create");
    source.exec_raw_for_test(&insert).await.expect("insert");
    table
}

/// 1a. The data-loss regression, pinned with CONCRETE expected values. Every wide type here reaches
///     `cell_to_value` on an Arrow variant that had NO explicit arm, so the old `_ => Null` dropped it
///     to `null` — invisible data loss in a panel. After the stringify fix each carries its real
///     value. This does NOT compare against the DataFusion path, because that path REFUSES a table
///     containing `jsonb` outright ("unsupported data type: jsonb") — the direct path is strictly
///     better here, so DataFusion is not a valid oracle for these types (1b covers the types it can
///     handle). FAILS against `_ => Null` (every wide cell comes back null); passes after the fix.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_path_preserves_wide_types() {
    let Some(source) = try_connect().await else {
        return;
    };
    let table = seed_wide_types(source.as_ref()).await;
    let sql = format!(
        "SELECT c_jsonb, c_uuid, c_num, c_arr, c_bytea, c_ts \
         FROM {table} ORDER BY c_uuid NULLS LAST"
    );
    let out = run_direct(source.as_ref(), &sql).await;
    assert_eq!(out.rows.len(), 2, "seeded one value row + one all-null row");

    let value = out.rows[0].as_array().unwrap();
    // Column order matches the SELECT. Each was a `null` under the old catch-all.
    let by = |name: &str| {
        let i = out.columns.iter().position(|c| c == name).unwrap();
        value[i].clone()
    };
    // jsonb → its JSON text (a real object payload, not null).
    assert_eq!(
        by("c_jsonb").as_str().unwrap(),
        "{\"k\": 42}",
        "jsonb dropped"
    );
    // uuid → the canonical uuid string.
    assert_eq!(
        by("c_uuid").as_str().unwrap(),
        "11111111-2222-3333-4444-555555555555",
        "uuid dropped"
    );
    // numeric(20,4) fits Decimal128 → a real number.
    assert!(
        (by("c_num").as_f64().unwrap() - 3.1416).abs() < 1e-9,
        "numeric dropped: {:?}",
        by("c_num")
    );
    // int[] → a readable array rendering (the exact text form is the connector's; non-null is the
    // property that matters — before the fix this vanished).
    assert!(!by("c_arr").is_null(), "int[] dropped");
    assert!(
        by("c_arr").as_str().unwrap().contains('1') && by("c_arr").as_str().unwrap().contains('3'),
        "int[] should render its elements: {:?}",
        by("c_arr")
    );
    // bytea → its hex rendering.
    assert!(
        by("c_bytea").as_str().unwrap().contains("deadbeef"),
        "bytea dropped: {:?}",
        by("c_bytea")
    );
    // timestamptz → a real timestamp string.
    assert!(
        by("c_ts").as_str().unwrap().contains("2026-01-02"),
        "timestamptz dropped: {:?}",
        by("c_ts")
    );

    // The all-null row stays all-null — the fix must NOT fabricate a value for a genuinely-null cell.
    let null_row = out.rows[1].as_array().unwrap();
    for (c, col) in out.columns.iter().enumerate() {
        assert!(
            null_row[c].is_null(),
            "a genuinely-null cell was fabricated at col {col}: {:?}",
            null_row[c]
        );
    }

    let _ = source
        .exec_raw_for_test(&format!("DROP TABLE {table}"))
        .await;
}

/// 1b. Direct-vs-DataFusion parity on the types BOTH paths can carry (int/float/text/bool/
///     timestamptz). This is the "the fast path didn't change the answer" guarantee for the common
///     case: the direct path must return exactly what the DataFusion path returns, cell for cell.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_path_matches_datafusion_for_common_types() {
    let Some(source) = try_connect().await else {
        return;
    };
    // A table of ONLY common types — the DataFusion per-table provider builds a schema for the WHOLE
    // table, so a jsonb/uuid column anywhere in it would make the provider itself fail (1a's table).
    // Here both paths can carry every column, so they must agree cell for cell.
    let table = format!("lb_common_{}", std::process::id());
    source
        .exec_raw_for_test(&format!("DROP TABLE IF EXISTS {table}"))
        .await
        .expect("drop");
    source
        .exec_raw_for_test(&format!(
            "CREATE TABLE {table} (c_int int, c_float float8, c_text text, c_bool bool, c_ts timestamptz)"
        ))
        .await
        .expect("create");
    source
        .exec_raw_for_test(&format!(
            "INSERT INTO {table} VALUES \
               (7, 1.5, 'hello', true, '2026-01-02T03:04:05Z'::timestamptz), \
               (9, 2.5, 'world', false, '2026-02-03T04:05:06Z'::timestamptz)"
        ))
        .await
        .expect("insert");
    let sql = format!("SELECT c_int, c_float, c_text, c_bool, c_ts FROM {table} ORDER BY c_int");
    let direct = run_direct(source.as_ref(), &sql).await;
    let df = run_datafusion(source.as_ref(), &sql).await;

    assert_eq!(direct.columns, df.columns, "columns diverge");
    assert_eq!(
        direct.rows, df.rows,
        "direct path changed a common-type answer"
    );

    let _ = source
        .exec_raw_for_test(&format!("DROP TABLE {table}"))
        .await;
}

/// 2. Remote LIMIT — the direct path bounds the result to ROW_CAP by pushing a LIMIT into the source,
///    not by fetching everything and truncating. Seeds ROW_CAP + 50 rows; asserts exactly ROW_CAP
///    come back AND that the plan Postgres actually ran carries a `Limit` node.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_path_pushes_limit_into_source() {
    let Some(source) = try_connect().await else {
        return;
    };
    let table = format!("lb_cap_{}", std::process::id());
    source
        .exec_raw_for_test(&format!("DROP TABLE IF EXISTS {table}"))
        .await
        .expect("drop");
    source
        .exec_raw_for_test(&format!("CREATE TABLE {table} (id int)"))
        .await
        .expect("create");
    source
        .exec_raw_for_test(&format!(
            "INSERT INTO {table} SELECT g FROM generate_series(1, {}) g",
            ROW_CAP + 50
        ))
        .await
        .expect("seed rows");

    // (a) The result is clamped to exactly ROW_CAP.
    let out = run_direct(source.as_ref(), &format!("SELECT id FROM {table}")).await;
    assert_eq!(
        out.rows.len(),
        ROW_CAP,
        "direct path must clamp to ROW_CAP via a remote LIMIT"
    );

    // (b) The bound was PUSHED into the source: EXPLAIN the exact SQL the direct path sends and
    //     assert the executed plan has a Limit node. `cap_direct_sql` is the wrap the direct path
    //     applies; run EXPLAIN over it through the same raw seam.
    let capped = validate::cap_direct_sql(&format!("SELECT id FROM {table}"));
    let explain = source
        .explain_for_test(&capped)
        .await
        .expect("explain capped sql");
    assert!(
        explain.to_lowercase().contains("limit"),
        "expected a Limit node in the executed plan (bound pushed into the source); got:\n{explain}"
    );

    // (c) A user-supplied smaller LIMIT still wins (the outer cap clamps, never widens).
    let small = run_direct(source.as_ref(), &format!("SELECT id FROM {table} LIMIT 5")).await;
    assert_eq!(
        small.rows.len(),
        5,
        "a smaller user LIMIT must win over ROW_CAP"
    );

    let _ = source
        .exec_raw_for_test(&format!("DROP TABLE {table}"))
        .await;
}

/// 3. Routing — a NORMAL dashboard panel query (a plain SELECT, no `information_schema`) is marked
///    `is_simple` and takes the direct path through the REAL routing function `register_and_run` (the
///    exact code the `federation.query` tool — hence a `viz.query` panel target — runs). Proven by the
///    phase timings: the DataFusion-only phases (`info_schema_reg_ms`, `table_reg_ms`, `plan_ms`) are
///    all ZERO, while the direct `execute` phase ran. This is the "dashboard uses the direct path by
///    default, automatically per-query" guarantee — no toggle, no host-side forcing. An
///    `information_schema` probe (the ONE case that still needs DataFusion) is checked to be NON-simple
///    alongside, so the split is exactly where it should be.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn normal_panel_query_takes_direct_path_no_datafusion_phases() {
    let Some(source) = try_connect().await else {
        return;
    };
    let table = format!("lb_route_{}", std::process::id());
    source
        .exec_raw_for_test(&format!("DROP TABLE IF EXISTS {table}"))
        .await
        .expect("drop");
    source
        .exec_raw_for_test(&format!("CREATE TABLE {table} (id int, val float8)"))
        .await
        .expect("create");
    source
        .exec_raw_for_test(&format!(
            "INSERT INTO {table} SELECT g, g * 1.5 FROM generate_series(1, 100) g"
        ))
        .await
        .expect("seed");

    // The shape a viz panel sends: a plain SELECT over one table with WHERE/ORDER/aggregate — never
    // information_schema. It must route through the direct path.
    let panel_sql = format!("SELECT id, val FROM {table} WHERE val > 10 ORDER BY id");
    assert!(
        validate_select(&panel_sql).unwrap().is_simple,
        "a normal panel query must be is_simple (direct path)"
    );

    let (result, phases) = query::run_with_phases_for_test(source.as_ref(), &panel_sql)
        .await
        .expect("run panel query");
    assert!(!result.rows.is_empty(), "panel query returned rows");

    // The crux: NO DataFusion plan/table-registration phases ran on this path.
    assert_eq!(
        phases.info_schema_reg_ms, 0,
        "no info_schema registration on direct path"
    );
    assert_eq!(
        phases.table_reg_ms, 0,
        "no per-table provider registration on direct path"
    );
    assert_eq!(phases.plan_ms, 0, "no DataFusion planning on direct path");

    // And the contrast: an information_schema probe — the ONE thing that still (correctly) needs
    // DataFusion — is NOT simple, so it would take the federated path.
    assert!(
        !validate_select("SELECT table_name FROM information_schema.tables")
            .unwrap()
            .is_simple,
        "an information_schema probe must stay on the DataFusion path"
    );

    let _ = source
        .exec_raw_for_test(&format!("DROP TABLE {table}"))
        .await;
}

/// A pure-unit check on the SQL wrap (no DB): the cap wrap nests the query and appends ROW_CAP, and
/// is idempotent enough that a user LIMIT survives inside. Runs even without Postgres.
#[test]
fn cap_wrap_shape() {
    let capped = validate::cap_direct_sql("SELECT id FROM t LIMIT 5");
    assert!(
        capped.contains("SELECT id FROM t LIMIT 5"),
        "inner query preserved"
    );
    assert!(
        capped.trim_end().ends_with(&format!("LIMIT {ROW_CAP}")),
        "outer ROW_CAP appended: {capped}"
    );
}

/// A MEASUREMENT, not an assertion — `#[ignore]`d so it never runs in the normal suite (it's a
/// timing, and timings don't belong in a pass/fail gate). Run explicitly:
///   `cargo test -p federation --features postgres --test direct_path_pg_test --
///    perf_direct_vs_datafusion --ignored --nocapture`
/// It seeds a 10K-row COMMON-types table (both paths can carry it), warms each path once (pool +
/// provider cache), then times ONE identical query through each and prints the real elapsed_ms. The
/// session doc records the measured numbers — not the PR's narrated ones.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "perf measurement; run explicitly with --ignored"]
async fn perf_direct_vs_datafusion() {
    let Some(source) = try_connect().await else {
        return;
    };
    let table = format!("lb_perf_{}", std::process::id());
    source
        .exec_raw_for_test(&format!("DROP TABLE IF EXISTS {table}"))
        .await
        .expect("drop");
    source
        .exec_raw_for_test(&format!(
            "CREATE TABLE {table} (id int, val float8, label text, ok bool, ts timestamptz)"
        ))
        .await
        .expect("create");
    source
        .exec_raw_for_test(&format!(
            "INSERT INTO {table} SELECT g, g * 1.5, 'row-' || g, g % 2 = 0, \
             now() - (g || ' seconds')::interval FROM generate_series(1, 10000) g"
        ))
        .await
        .expect("seed 10k");

    let sql = format!("SELECT id, val, label, ok, ts FROM {table} ORDER BY id");

    // Warm BOTH paths once (connection pool + DataFusion provider cache) so the timing is steady-state
    // query cost, not first-touch setup.
    let _ = run_direct(source.as_ref(), &sql).await;
    let _ = run_datafusion(source.as_ref(), &sql).await;

    let t = std::time::Instant::now();
    let d = run_direct(source.as_ref(), &sql).await;
    let direct_ms = t.elapsed().as_millis();

    let t = std::time::Instant::now();
    let f = run_datafusion(source.as_ref(), &sql).await;
    let df_ms = t.elapsed().as_millis();

    eprintln!(
        "PERF 10k-row warm query: direct={direct_ms}ms ({} rows) | datafusion={df_ms}ms ({} rows)",
        d.rows.len(),
        f.rows.len()
    );

    let _ = source
        .exec_raw_for_test(&format!("DROP TABLE {table}"))
        .await;
}
