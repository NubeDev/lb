//! `run_query` — the engine-agnostic orchestration of a federated read (datasources scope). It
//! validates SELECT-only, connects the source (DSN handed in, never stored/logged), registers each
//! referenced table as a DataFusion `TableProvider`, runs the query through a `SessionContext`, and
//! returns `{columns, rows}` bounded by the row cap. The pattern (embed the engine, register
//! per-table providers, run validated SQL) is adapted from rubix-cube (MIT/Apache-2.0).
//!
//! `discover_*` — the `federation.schema` discovery path: reuses the SAME table-provider factory to
//! read each source's own catalog (Postgres `pg_catalog` / SQLite `sqlite_master`) and to read a
//! table's Arrow schema for columns. It does NOT issue `information_schema` SQL (the engine registers
//! only the tables a query references, so a virtual catalog is unreachable); it goes through the real
//! remote engine the provider pushes down to.
//!
//! **Pushdown** (federation-pushdown scope): the `SessionContext` is built from
//! `datafusion_federation::default_session_state()` — the federation optimizer rule + query planner
//! installed in the SessionState. Each registered table is the per-engine *federated* provider (a
//! `FederatedTableProviderAdaptor` over the real `SqlTable`). With both in place, DataFusion detects
//! that every table in a single-source plan shares one compute context, **unparses the plan back to
//! that engine's SQL dialect, and executes it remotely** — only the (typically small) result batches
//! cross the provider boundary. The `df.limit(0, Some(ROW_CAP))` then unparses to a remote LIMIT,
//! which is strictly better than the previous client-side cap. A plan the unparser can't push down
//! (mixed `information_schema` views + user tables, exotic constructs) falls back to per-table scans
//! — today's behavior, still correct.

use arrow::record_batch::RecordBatch;
use datafusion::prelude::SessionContext;
use datafusion::sql::TableReference;
use serde_json::Value;

use crate::event::{query_event, Cache, Outcome};
use crate::pool::{cached_connect, evict, is_warm};
use crate::source::{connect, ColumnMeta, Source, SourceError, TableMeta};
use crate::validate::{validate_select, ROW_CAP};

/// How long a single federated query may run before it is abandoned and its pool evicted
/// (federation-pool-cache scope). There was previously NO bound on any query path — one unbounded
/// remote query hung for >2 minutes and starved every other source in the child, including local
/// SQLite, until a restart. 30 s is sized to "slow remote that is still working"; a dashboard tile
/// would rather fail sooner, so callers may pass a shorter bound per call.
pub const DEFAULT_QUERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// A fresh `SessionContext` with the federation optimizer rule + query planner installed
/// (federation-pushdown scope). One per call — federation decisions are made per-plan from the
/// registered providers' compute contexts; no cross-call state is shared.
fn federated_context() -> SessionContext {
    SessionContext::new_with_state(datafusion_federation::default_session_state())
}

/// The result of a federated query: the column names and the rows (each an array of JSON cells,
/// column-aligned). Bounded to [`ROW_CAP`] rows.
#[derive(Debug)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
}

/// Run `sql` against the `kind` source at `dsn`. Validates SELECT-only first, registers only the
/// tables the query references, and caps the result. The DSN lives only inside the pool.
///
/// The connected source comes from the warm-pool cache, so the connect cost is paid once per source
/// per child lifetime instead of once per query. The whole thing is bounded by
/// [`DEFAULT_QUERY_TIMEOUT`]; on elapse the pool is evicted (it is suspect) and a typed error is
/// returned, so one hung remote cannot occupy the child.
pub async fn run_query(kind: &str, dsn: &str, sql: &str) -> Result<QueryResult, String> {
    run_query_with(kind, dsn, sql, None, DEFAULT_QUERY_TIMEOUT).await
}

/// [`run_query`] with an explicit bound and the host-side datasource `source` name for events.
/// `source` is an opaque label, never a DSN.
pub async fn run_query_with(
    kind: &str,
    dsn: &str,
    sql: &str,
    source_name: Option<&str>,
    timeout: std::time::Duration,
) -> Result<QueryResult, String> {
    let validated = validate_select(sql).map_err(|e| e.to_string())?;

    // Sampled BEFORE the connect, so the event reports what this call actually did rather than the
    // state it leaves behind (which is always "warm").
    let cache = if is_warm(kind, dsn) {
        Cache::Hit
    } else {
        Cache::Miss
    };
    let started = std::time::Instant::now();

    let bounded = tokio::time::timeout(timeout, async {
        let source = cached_connect(kind, dsn).await.map_err(|e| e.to_string())?;
        register_and_run(source.as_ref(), &validated, sql).await
    })
    .await;

    let elapsed = started.elapsed().as_millis();
    match bounded {
        Ok(Ok(result)) => {
            let rows = result.rows.len();
            query_event(source_name, kind, cache, sql, elapsed, &Outcome::Ok(rows));
            Ok(result)
        }
        Ok(Err(e)) => {
            query_event(
                source_name,
                kind,
                cache,
                sql,
                elapsed,
                &Outcome::Error(e.clone()),
            );
            Err(e)
        }
        Err(_elapsed) => {
            // Scope Risk 3: a pool that hung is suspect. Without this eviction a poisoned entry
            // would serve failures for the child's lifetime, where per-call connect self-healed —
            // i.e. caching would be strictly WORSE than the behaviour it replaced.
            evict(kind, dsn);
            query_event(source_name, kind, cache, sql, elapsed, &Outcome::Timeout);
            Err(format!(
                "query exceeded the {}s bound and was cancelled; the connection was dropped",
                timeout.as_secs()
            ))
        }
    }
}

/// A real connectivity probe for the `kind` source at `dsn` — `Ok(())` is green.
///
/// Deliberately BYPASSES the warm-pool cache and uses a fresh `connect`: a probe that reused a
/// cached pool would no longer prove that a new connection can be established, which is the entire
/// question `datasource.test` is asked. It also evicts on failure, so a probe doubles as the manual
/// "this source is broken, drop what you're holding" lever.
pub async fn probe(kind: &str, dsn: &str) -> Result<(), String> {
    let result = async {
        let source = connect(kind, dsn).await.map_err(|e| e.to_string())?;
        source.probe().await.map_err(|e| e.to_string())
    }
    .await;
    if result.is_err() {
        evict(kind, dsn);
    }
    result
}

/// Register each referenced table into a fresh `SessionContext` (plus any synthesized
/// `information_schema` views the query reads), run the SQL, and shape the result.
async fn register_and_run(
    source: &dyn Source,
    validated: &crate::validate::ValidatedSelect,
    sql: &str,
) -> Result<QueryResult, String> {
    let ctx = federated_context();
    crate::info_schema::register_information_schema(
        &ctx,
        source,
        validated.wants_info_tables,
        validated.wants_info_columns,
    )
    .await?;
    for table in &validated.tables {
        let reference = TableReference::bare(table.clone());
        let provider = source
            .table_provider(&reference)
            .await
            .map_err(|e| e.to_string())?;
        ctx.register_table(reference, provider)
            .map_err(|e| format!("register {table}: {e}"))?;
    }

    let df = ctx.sql(sql).await.map_err(|e| format!("plan: {e}"))?;
    // Cap before collect: under pushdown this unparses to a remote LIMIT executed in the source
    // engine (strictly better than the prior client-side cap); under fallback it still caps the
    // collected batches. An unbounded export is a mirror job, never a live query (§6.1).
    let df = df.limit(0, Some(ROW_CAP)).map_err(|e| e.to_string())?;
    let batches = df.collect().await.map_err(|e| format!("execute: {e}"))?;
    shape(batches)
}

/// Convert collected Arrow batches into `{columns, rows}`. Columns come from the first batch's
/// schema; rows are JSON objects flattened to column-aligned arrays.
pub(crate) fn shape(batches: Vec<RecordBatch>) -> Result<QueryResult, String> {
    let columns: Vec<String> = match batches.first() {
        Some(b) => b
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect(),
        None => Vec::new(),
    };

    // arrow-json writes each row as a JSON object keyed by column name; re-project to a
    // column-aligned array so the wire shape is `{columns:[...], rows:[[...], ...]}`.
    let mut buf = Vec::new();
    {
        let mut writer = arrow_json::ArrayWriter::new(&mut buf);
        for batch in &batches {
            writer.write(batch).map_err(|e| e.to_string())?;
        }
        writer.finish().map_err(|e| e.to_string())?;
    }
    let objs: Vec<Value> = if buf.is_empty() {
        Vec::new()
    } else {
        serde_json::from_slice(&buf).map_err(|e| e.to_string())?
    };

    let rows: Vec<Value> = objs
        .into_iter()
        .map(|obj| {
            Value::Array(
                columns
                    .iter()
                    .map(|c| obj.get(c).cloned().unwrap_or(Value::Null))
                    .collect(),
            )
        })
        .collect();

    Ok(QueryResult { columns, rows })
}

// ───────────────────────────── discovery (`federation.schema`) ─────────────────────────────

/// Run a discovery SELECT that reads catalog tables, returning JSON OBJECT rows (keyed by column
/// name). Each `(alias, remote)` binding builds a provider for the remote catalog table (the same
/// factory `probe` uses) and registers it under the bare `alias`, so the SQL references the alias —
/// this decouples DataFusion's name resolution from the remote catalog's dotted names.
pub(crate) async fn catalog_rows(
    source: &dyn Source,
    sql: &str,
    bindings: &[(&str, &str)],
) -> Result<Vec<Value>, String> {
    let ctx = federated_context();
    for (alias, remote) in bindings {
        // `parse_str` for the remote: catalog names are dotted (`pg_catalog.pg_tables`) and must
        // split into schema + table so the provider introspects the real catalog (a `bare` dotted
        // name reports an empty schema). The alias is a single bare identifier the SQL references.
        let provider = source
            .table_provider(&TableReference::parse_str(remote))
            .await
            .map_err(|e| e.to_string())?;
        ctx.register_table(TableReference::bare(*alias), provider)
            .map_err(|e| format!("register {alias}: {e}"))?;
    }
    let df = ctx.sql(sql).await.map_err(|e| format!("plan: {e}"))?;
    let df = df.limit(0, Some(ROW_CAP)).map_err(|e| e.to_string())?;
    let batches = df.collect().await.map_err(|e| format!("execute: {e}"))?;
    rows_as_objects(batches)
}

/// Collect Arrow batches into JSON OBJECT rows keyed by column name (the discovery result shape).
fn rows_as_objects(batches: Vec<RecordBatch>) -> Result<Vec<Value>, String> {
    let mut buf = Vec::new();
    {
        let mut writer = arrow_json::ArrayWriter::new(&mut buf);
        for batch in &batches {
            writer.write(batch).map_err(|e| e.to_string())?;
        }
        writer.finish().map_err(|e| e.to_string())?;
    }
    if buf.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_slice(&buf).map_err(|e| e.to_string())
}

/// Discover the user tables in the source via its own catalog. The list SQL + bindings are
/// per-source-kind; the orchestration is shared.
pub async fn discover_tables(kind: &str, dsn: &str) -> Result<Vec<TableMeta>, String> {
    let source = cached_connect(kind, dsn).await.map_err(|e| e.to_string())?;
    source.list_tables().await.map_err(|e| e.to_string())
}

/// Discover one table's columns by reading its `TableProvider` Arrow schema — engine-agnostic (works
/// for Postgres and SQLite alike; the provider pushes down and reports the real remote schema).
pub async fn describe_table(kind: &str, dsn: &str, table: &str) -> Result<Vec<ColumnMeta>, String> {
    let source = cached_connect(kind, dsn).await.map_err(|e| e.to_string())?;
    let provider = source
        .table_provider(&TableReference::bare(table))
        .await
        .map_err(|e| e.to_string())?;
    let schema = provider.schema();
    let cols = schema
        .fields()
        .iter()
        .map(|f| ColumnMeta {
            name: f.name().clone(),
            data_type: f.data_type().to_string(),
            nullable: f.is_nullable(),
        })
        .collect();
    Ok(cols)
}

/// Build `TableMeta` rows from JSON objects (a `{name, rows?}` shape, tolerant of missing rows).
pub fn table_meta_from_rows(rows: Vec<Value>) -> Vec<TableMeta> {
    rows.into_iter()
        .filter_map(|obj| {
            let name = obj.get("name")?.as_str()?.to_string();
            let rows = obj
                .get("rows")
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|n| n as i64)));
            Some(TableMeta { name, rows })
        })
        .collect()
}

// Provide the per-kind list query so each `Source` impl stays small and the catalog SQL lives in one
// place. Returns `(sql, bindings)`.
pub(crate) fn list_tables_plan(
    kind: &str,
) -> Result<(&'static str, Vec<(&'static str, &'static str)>), String> {
    match kind {
        "sqlite" => Ok((
            "SELECT name AS name FROM __sm__ WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            vec![("__sm__", "sqlite_master")],
        )),
        // List names from `pg_tables` only. The earlier `reltuples` estimate joined `pg_class`, but
        // the pushed-down `pg_class` provider doesn't expose `relname` to the DataFusion plan
        // (`No field named c.relname`), which broke the whole listing. Names are what the browse panel
        // needs; a row estimate is a nice-to-have we drop rather than fail the list over.
        "postgres" | "timescale" => Ok((
            "SELECT tablename AS name FROM __pg_tables__ WHERE schemaname = 'public' ORDER BY tablename",
            vec![("__pg_tables__", "pg_catalog.pg_tables")],
        )),
        other => Err(format!("unknown source kind: {other}")),
    }
}

/// Run the per-kind list query via the shared catalog runner. Used by `Source::list_tables` impls so
/// they share one orchestration path.
pub async fn run_list_tables(
    source: &dyn Source,
    kind: &str,
) -> Result<Vec<TableMeta>, SourceError> {
    let (sql, bindings) = list_tables_plan(kind).map_err(SourceError)?;
    let rows = catalog_rows(source, sql, &bindings)
        .await
        .map_err(SourceError)?;
    Ok(table_meta_from_rows(rows))
}

#[cfg(test)]
mod tests {
    //! Pushdown-correctness tests (federation-pushdown scope) against a REAL seeded SQLite file
    //! (no Docker, no mocks — the source layer is the one sanctioned fake-boundary, testing §0).
    //! These pin the three things that could have broken under pushdown:
    //!   1. A demo-shaped JOIN + GROUP BY + ORDER BY returns the exact same `{columns, rows}` it did
    //!      under per-table scans (correctness heart).
    //!   2. Bare `COUNT(*)` works under pushdown — the previous "Physical input schema should be the
    //!      same" steer is gone; if a future provider upgrade brings the bug back, this fires.
    //!   3. The federation optimizer recognizes a single-source multi-table plan as ONE federated
    //!      scan node in the EXPLAIN output (structural — not a flaky timing assertion).
    //! Plus the ROW_CAP remote-LIMIT clamp and the SELECT-only regression set.

    use super::*;
    use crate::source::{connect, Source};
    use crate::validate::validate_select;
    use datafusion::sql::TableReference;

    /// Seed a real `.db` with the demo's shape: `site` (parents) joined to `point_reading`
    /// (children), plus enough rows to make per-table-scan cost obvious. Returns the file path (the
    /// SQLite DSN). Each call gets a UNIQUE file (atomic counter + pid) so parallel `cargo test`
    /// workers don't collide on the seeded schema (`table site already exists`).
    fn seed_demo_db(row_count: usize) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "lb-fed-pushdown-{seq}-{}-{}.db",
            row_count,
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let conn = rusqlite::Connection::open(&path).expect("open sqlite fixture");
        conn.execute_batch(
            "CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE point_reading (
               time TEXT, point_id TEXT, value REAL, site_id TEXT REFERENCES site(id)
             );",
        )
        .expect("create schema");
        // Two sites, then `row_count` readings split between them (group-able to two rows).
        conn.execute(
            "INSERT INTO site VALUES ('site-001','Northside Factory'),('site-002','City Tower')",
            [],
        )
        .expect("seed sites");
        // Generate `row_count` readings as a single INSERT with bound params (fast even at 20k).
        let half = row_count / 2;
        for site in ["site-001", "site-002"] {
            for i in 0..half {
                // Alternate values so AVG differs per site (catches a wrong grouping immediately).
                let v = if i % 2 == 0 { 1.5 } else { 2.5 };
                conn.execute(
                    "INSERT INTO point_reading VALUES ('2026-01-01', ?1, ?2, ?3)",
                    rusqlite::params![format!("{site}-p{i}"), v, site],
                )
                .expect("seed reading");
            }
        }
        path.to_string_lossy().into_owned()
    }

    async fn demo_source(row_count: usize) -> (String, std::sync::Arc<dyn Source>) {
        let dsn = seed_demo_db(row_count);
        let source = connect("sqlite", &dsn).await.expect("connect sqlite");
        (dsn, source)
    }

    /// Register the source's referenced tables (the same path `run_query` takes) into a federation-
    /// enabled context, then run `sql` and return the shaped result. Used by the structural tests.
    async fn run_via_federated(source: &dyn Source, sql: &str) -> QueryResult {
        let validated = validate_select(sql).expect("validate");
        register_and_run(source, &validated, sql)
            .await
            .expect("run")
    }

    /// 1. Correctness heart — the demo-shaped JOIN + GROUP BY + ORDER BY returns the exact expected
    ///    `{columns, rows}` (the same shape the non-pushdown path returned; pinned here so a future
    ///    unparser drift is caught immediately).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn join_groupby_orderby_matches_expectation() {
        let (_dsn, source) = demo_source(40).await;
        let out = run_via_federated(
            source.as_ref(),
            "SELECT s.name AS site, AVG(r.value) AS avg_val \
             FROM point_reading r JOIN site s ON r.site_id = s.id \
             GROUP BY s.name \
             ORDER BY s.name",
        )
        .await;
        assert_eq!(out.columns, vec!["site".to_string(), "avg_val".to_string()]);
        // Two aggregated rows, alphabetically ordered, both AVGs are the mean of {1.5, 2.5} = 2.0.
        assert_eq!(out.rows.len(), 2, "two sites → two groups: {out:?}");
        assert_eq!(out.rows[0][0].as_str().unwrap(), "City Tower");
        assert_eq!(out.rows[1][0].as_str().unwrap(), "Northside Factory");
        for row in &out.rows {
            let v = row[1].as_f64().unwrap();
            assert!((v - 2.0).abs() < 1e-9, "AVG should be 2.0, got {v}");
        }
    }

    /// 2. `COUNT(*)` wrinkle — bare `COUNT(*)` was rejected under per-table scans (upstream provider
    ///    bug). Under pushdown it unparses to remote SQL and returns the row count cleanly. If a
    ///    future upgrade breaks this again, this test fires (and we re-introduce the steer).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bare_count_star_works_under_pushdown() {
        let (_dsn, source) = demo_source(40).await;
        let out =
            run_via_federated(source.as_ref(), "SELECT COUNT(*) AS n FROM point_reading").await;
        assert_eq!(out.columns, vec!["n".to_string()]);
        assert_eq!(out.rows.len(), 1);
        let n = out.rows[0][0]
            .as_i64()
            .or_else(|| out.rows[0][0].as_f64().map(|x| x as i64))
            .unwrap();
        assert_eq!(
            n, 40,
            "COUNT(*) returns the row count under pushdown: {out:?}"
        );
    }

    /// 3. Structural — a multi-table single-source query plans as ONE federated scan (not a flaky
    ///    timing assertion). The optimized plan's pretty-printed form contains a `FederatedScan`
    ///    node when the federation optimizer succeeded; if it fell back to per-table scans the plan
    ///    would show `TableScan`/`Projection` over individual base tables.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn plan_is_one_federated_scan() {
        let (_dsn, source) = demo_source(40).await;
        // Manually walk the same register path `register_and_run` uses, then ask for the EXPLAIN.
        let ctx = federated_context();
        let sql = "SELECT s.name, AVG(r.value) FROM point_reading r JOIN site s ON r.site_id = s.id GROUP BY s.name";
        let validated = validate_select(sql).expect("validate");
        for t in &validated.tables {
            let reference = TableReference::bare(t.clone());
            let provider = source.table_provider(&reference).await.unwrap();
            ctx.register_table(reference, provider).unwrap();
        }
        // `EXPLAIN` returns rows shaped `(plan_type: Utf8, plan: Utf8)` — the plan column carries
        // the formatted logical+physical plan. Concatenate every cell of every row so we don't miss
        // the federated node whichever row it lands in.
        let batches = ctx
            .sql(&format!("EXPLAIN {sql}"))
            .await
            .unwrap()
            .collect()
            .await
            .unwrap();
        let mut full = String::new();
        for batch in &batches {
            use arrow::array::AsArray;
            if let Some(col) = batch.column_by_name("plan") {
                if let Some(arr) = col.as_string_opt::<i32>() {
                    for v in arr.iter().flatten() {
                        full.push_str(v);
                        full.push('\n');
                    }
                }
            }
        }
        assert!(
            full.contains("VirtualExecutionPlan") && full.contains("compute_context"),
            "expected a federated physical node (VirtualExecutionPlan + compute_context) in the \
             EXPLAIN; pushdown didn't fire. Got:\n{full}"
        );
        // The unparser emitted ONE remote SQL statement covering both tables + the JOIN + GROUP BY
        // (rather than two per-table SQL round-trips). `base_sql=` is the federation adaptor's
        // marker for the unparsed statement.
        assert!(
            full.contains("base_sql="),
            "expected an unparsed `base_sql=` statement; got:\n{full}"
        );
        // And the unparsed SQL must reference BOTH base tables (the whole point of statement-level
        // pushdown vs per-table scans — the JOIN happens in the source engine, not the sidecar).
        let base_sql_line = full
            .lines()
            .find(|l| l.contains("base_sql="))
            .expect("base_sql line");
        assert!(
            base_sql_line.contains("point_reading") && base_sql_line.contains("site"),
            "unparsed SQL should reference both tables: {base_sql_line}"
        );
    }

    /// 4. ROW_CAP — when the source holds more than ROW_CAP rows, the remote LIMIT the unparser
    ///    emits clamps the result to exactly ROW_CAP. Seeds ROW_CAP + 50 rows.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn row_cap_clamps_via_remote_limit() {
        let (_dsn, source) = demo_source(ROW_CAP + 50).await;
        let out = run_via_federated(source.as_ref(), "SELECT point_id FROM point_reading").await;
        assert_eq!(
            out.rows.len(),
            ROW_CAP,
            "remote LIMIT clamps to exactly ROW_CAP"
        );
    }

    /// 5. SELECT-only regression — writes, DDL, and multi-statement inputs still refuse identically
    ///    (no contract change from pushdown — both gates still validate before planning).
    #[test]
    fn select_only_gates_unchanged() {
        for bad in [
            "INSERT INTO point_reading VALUES ('x','p',1.0,'site-001')",
            "UPDATE point_reading SET value = 0",
            "DELETE FROM point_reading",
            "DROP TABLE point_reading",
            "SELECT 1; SELECT 2",
        ] {
            assert!(validate_select(bad).is_err(), "should reject: {bad}");
        }
    }
}
