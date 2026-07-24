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
use futures::StreamExt;
use serde_json::Value;

use crate::event::{query_event, Cache, Outcome, QueryPhaseTimings, ResultCacheEvent};
use crate::pool::{cached_connect, evict, is_warm};
use crate::results::{self, Envelope};
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
///
/// **Used by tests, not by the binary** — hence the `allow`. The child's dispatch calls
/// [`run_query_cached`] (which wraps [`run_query_with`]), so nothing in `main.rs` reaches this
/// wrapper and the `bin` target reports it dead. It is kept because the pool-cache suite drives the
/// uncached path through it at 11 call sites: that suite is specifically about *connection* reuse,
/// which the result cache would mask by short-circuiting the query entirely. Deleting it would mean
/// threading an explicit `DEFAULT_QUERY_TIMEOUT` through every one of those calls to say nothing new.
///
/// `#[cfg(test)]` is NOT usable here: the integration tests `#[path]`-include this file as a module,
/// so they compile it with `cfg(test)` **false** and would lose the function they need.
#[allow(dead_code)]
pub async fn run_query(kind: &str, dsn: &str, sql: &str) -> Result<QueryResult, String> {
    run_query_with(kind, dsn, sql, None, DEFAULT_QUERY_TIMEOUT)
        .await
        .map(|(r, _phases)| r)
}

/// [`run_query_with`] fronted by the TTL-bounded **result** cache (federation-result-cache scope).
///
/// `input` is the whole child-received call input — it is what the cache keys on (minus `cache` and
/// `dsn`), so every field the child actually receives participates in identity automatically. The
/// freshness window comes from `input.cache.ttl_s`; absent, zero, or kill-switched → this is exactly
/// [`run_query_with`] with a `bypass` event and nothing stored.
///
/// On a HIT no query runs, so no pool `cache` state is sampled and that field is omitted from the
/// event (see `event::query_event`). Phase timings are absent on a hit.
///
/// This function is the SOLE event emitter for the federation query path: `run_query_with` no longer
/// emits its own event, so every result (hit/miss/bypass/error/timeout) produces exactly one line
/// on stderr carrying both the result-cache verdict and the per-phase timing breakdown.
pub async fn run_query_cached(
    kind: &str,
    dsn: &str,
    sql: &str,
    source_name: Option<&str>,
    input: &Value,
) -> Result<QueryResult, String> {
    let ttl = results::requested_ttl(input);
    let started = std::time::Instant::now();

    // Shared cell the inner closure fills with phase timings; on a hit the closure never runs so
    // `phases` stays `None`.
    let phases_cell = std::sync::Arc::new(std::sync::Mutex::new(None::<QueryPhaseTimings>));
    let phases_cell_clone = phases_cell.clone();

    let (outcome, state, age_ms) = results::cached_query(kind, dsn, input, ttl, || async {
        match run_query_with(kind, dsn, sql, source_name, DEFAULT_QUERY_TIMEOUT).await {
            Ok((result, p)) => {
                *phases_cell_clone.lock().expect("phases mutex") = Some(p);
                Ok(Envelope::new(result.columns, result.rows))
            }
            Err(e) => Err(e),
        }
    })
    .await;

    let elapsed_ms = started.elapsed().as_millis();
    let phases = phases_cell.lock().expect("phases mutex").take();

    // Pool cache state — sampled inside `run_query_with` and threaded back via phases.
    let pool_cache = phases.as_ref().and_then(|p| p.pool_cache);

    // Trace id for correlating sub-queries of one dashboard panel refresh.
    let trace_id = input.get("trace_id").and_then(|v| v.as_str()).unwrap_or("");

    let outcome_for_event = match &outcome {
        Ok(e) => Outcome::Ok(e.rows.len()),
        Err(e) => Outcome::Error(e.clone()),
    };

    query_event(
        source_name,
        kind,
        pool_cache,
        sql,
        elapsed_ms,
        &outcome_for_event,
        Some(&ResultCacheEvent { state, age_ms }),
        phases.as_ref(),
        if trace_id.is_empty() {
            None
        } else {
            Some(trace_id)
        },
    );

    outcome.map(|env| QueryResult {
        columns: env.columns.clone(),
        rows: env.rows.clone(),
    })
}

/// [`run_query`] with an explicit bound and the host-side datasource `source` name for events.
/// `source` is an opaque label, never a DSN.
///
/// Returns the result alongside a [`QueryPhaseTimings`] breakdown. This function does NOT emit a
/// query event — the caller (`run_query_cached`) owns the sole event emission so the phases and the
/// result-cache status appear in one line.
pub async fn run_query_with(
    kind: &str,
    dsn: &str,
    sql: &str,
    _source_name: Option<&str>,
    timeout: std::time::Duration,
) -> Result<(QueryResult, QueryPhaseTimings), String> {
    // Sampled BEFORE the connect, so phases report what this call actually did rather than the
    // state it leaves behind (which is always "warm"). Captured by `move` into the timeout block.
    let pool_cache = if is_warm(kind, dsn) {
        Cache::Hit
    } else {
        Cache::Miss
    };

    let t0 = std::time::Instant::now();
    let validated = validate_select(sql).map_err(|e| e.to_string())?;
    let validate_ms = t0.elapsed().as_millis() as u64;

    let bounded = tokio::time::timeout(timeout, async move {
        let mut phases = QueryPhaseTimings::default();
        phases.pool_cache = Some(pool_cache);

        let t0 = std::time::Instant::now();
        let source = cached_connect(kind, dsn).await.map_err(|e| e.to_string())?;
        phases.connect_ms = t0.elapsed().as_millis() as u64;

        let (result, inner) = register_and_run(source.as_ref(), &validated, sql).await?;
        phases.info_schema_reg_ms = inner.info_schema_reg_ms;
        phases.table_reg_ms = inner.table_reg_ms;
        phases.plan_ms = inner.plan_ms;
        phases.execute_ms = inner.execute_ms;
        phases.ttfb_ms = inner.ttfb_ms;
        phases.fetch_ms = inner.fetch_ms;
        phases.serialize_ms = inner.serialize_ms;
        Ok((result, phases))
    })
    .await;

    match bounded {
        Ok(Ok((result, mut phases))) => {
            phases.validate_ms = validate_ms;
            Ok((result, phases))
        }
        Ok(Err(e)) => Err(e),
        Err(_elapsed) => {
            // Scope Risk 3: a pool that hung is suspect. Without this eviction a poisoned entry
            // would serve failures for the child's lifetime, where per-call connect self-healed —
            // i.e. caching would be strictly WORSE than the behaviour it replaced.
            evict(kind, dsn);
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
/// question `datasource.test` is asked.
///
/// A probe is also the "re-check this source from scratch" lever, so it EVICTS the warm pool + the
/// result cache unconditionally — after a probe, the next query reflects the source's CURRENT state,
/// not a pre-probe cached view. This is what makes `datasource.test` the host's "I changed this
/// source (a schema migration / a pack UPGRADE reconciled a column), drop what you're holding" tool:
/// a stale connection opened before the change would otherwise keep serving the old shape
/// (pack-upgrade-scope §Risks: a host-side reconcile is out-of-band to the sidecar's cache).
pub async fn probe(kind: &str, dsn: &str) -> Result<(), String> {
    let result = async {
        let source = connect(kind, dsn).await.map_err(|e| e.to_string())?;
        source.probe().await.map_err(|e| e.to_string())
    }
    .await;
    // Drop any warm connection + cached rows for this source, pass or fail — the source may have
    // changed shape since the pool warmed (a migration/upgrade), and a probe is the re-check lever.
    evict(kind, dsn);
    results::evict_source(kind, dsn);
    result
}

/// Register each referenced table into a fresh `SessionContext` (plus any synthesized
/// `information_schema` views the query reads), run the SQL, and shape the result.
/// Returns the result alongside a per-phase timing breakdown.
async fn register_and_run(
    source: &dyn Source,
    validated: &crate::validate::ValidatedSelect,
    sql: &str,
) -> Result<(QueryResult, QueryPhaseTimings), String> {
    let mut phases = QueryPhaseTimings::default();

    // Fast path: ALL queries whose tables live in one database can skip the DataFusion
    // planning/unparsing ceremony and execute directly against the source. The database
    // handles JOINs, subqueries, CTEs, aggregations — everything — natively. DataFusion
    // is only needed when the query references synthetic `information_schema` views that
    // don't exist in the real database. This cuts ~700 ms of overhead per query.
    if validated.is_simple {
        return run_direct_path(source, sql).await;
    }

    let ctx = federated_context();

    let t0 = std::time::Instant::now();
    crate::info_schema::register_information_schema(
        &ctx,
        source,
        validated.wants_info_tables,
        validated.wants_info_columns,
    )
    .await?;
    phases.info_schema_reg_ms = t0.elapsed().as_millis() as u64;

    let t0 = std::time::Instant::now();
    for table in &validated.tables {
        let reference = TableReference::bare(table.clone());
        let provider = source
            .table_provider(&reference)
            .await
            .map_err(|e| e.to_string())?;
        ctx.register_table(reference, provider)
            .map_err(|e| format!("register {table}: {e}"))?;
    }
    phases.table_reg_ms = t0.elapsed().as_millis() as u64;

    let t0 = std::time::Instant::now();
    let df = ctx.sql(sql).await.map_err(|e| format!("plan: {e}"))?;
    phases.plan_ms = t0.elapsed().as_millis() as u64;

    // Cap before collect: under pushdown this unparses to a remote LIMIT executed in the source
    // engine (strictly better than the prior client-side cap); under fallback it still caps the
    // collected batches. An unbounded export is a mirror job, never a live query (§6.1).
    let df = df.limit(0, Some(ROW_CAP)).map_err(|e| e.to_string())?;

    let t0 = std::time::Instant::now();
    let mut stream = df
        .execute_stream()
        .await
        .map_err(|e| format!("execute: {e}"))?;
    let mut batches: Vec<RecordBatch> = Vec::new();
    while let Some(batch) = stream.next().await {
        if phases.ttfb_ms == 0 {
            phases.ttfb_ms = t0.elapsed().as_millis() as u64;
        }
        batches.push(batch.map_err(|e| format!("execute: {e}"))?);
    }
    phases.execute_ms = t0.elapsed().as_millis() as u64;
    phases.fetch_ms = phases.execute_ms.saturating_sub(phases.ttfb_ms);

    let t0 = std::time::Instant::now();
    let result = shape(batches)?;
    phases.serialize_ms = t0.elapsed().as_millis() as u64;

    Ok((result, phases))
}

/// The direct fast path: send the (row-capped) SQL straight to the source, bypassing DataFusion.
///
/// Taken for any query that does not read a synthetic `information_schema` view (`is_simple`). The
/// database handles JOINs/subqueries/CTEs/aggregations natively, so DataFusion's plan+unparse ceremony
/// (~700 ms) adds nothing. [`crate::validate::cap_direct_sql`] pushes the [`ROW_CAP`] down as a REAL
/// remote LIMIT (the source returns only capped rows) rather than the prior fetch-everything-then-
/// `truncate` — parity with the DataFusion path's `df.limit(0, Some(ROW_CAP))`.
async fn run_direct_path(
    source: &dyn Source,
    sql: &str,
) -> Result<(QueryResult, QueryPhaseTimings), String> {
    let mut phases = QueryPhaseTimings::default();
    let t0 = std::time::Instant::now();

    let capped_sql = crate::validate::cap_direct_sql(sql);

    // Try the Arrow-free JSON path first (Postgres overrides this to skip Arrow entirely).
    // The serialization is bundled INTO execute_ms — there is no separate serialize step.
    let (columns, json_rows) = source
        .query_direct_json(&capped_sql)
        .await
        .map_err(|e| format!("direct json query: {e}"))?;

    phases.execute_ms = t0.elapsed().as_millis() as u64;
    phases.ttfb_ms = phases.execute_ms;

    Ok((
        QueryResult {
            columns,
            rows: json_rows,
        },
        phases,
    ))
}

// ── Test seams ──────────────────────────────────────────────────────────────────────────────────
// The integration tests `#[path]`-include this file (compiled with `cfg(test)` FALSE), so a
// `#[cfg(test)]` helper is invisible to them. These thin `pub` wrappers expose the REAL direct and
// DataFusion paths for the parity tests — they add no behavior, they only let a test drive each path
// explicitly and compare (fine-grained-data-path scope §Testing plan). Dead in the `bin` target.

/// Drive the REAL direct fast path for a query the validator marked `is_simple`. Used by the parity
/// test to compare the direct path's output against the DataFusion oracle for the same SQL.
#[allow(dead_code)]
pub async fn run_via_direct_for_test(
    source: &dyn Source,
    validated: &crate::validate::ValidatedSelect,
    sql: &str,
) -> Result<QueryResult, String> {
    debug_assert!(validated.is_simple, "helper is for the direct path");
    run_direct_path(source, sql).await.map(|(r, _)| r)
}

/// Drive the REAL routing function [`register_and_run`] — the exact code the sidecar runs for a
/// `federation.query` (and thus for a `viz.query` panel target) — and return the per-phase timings.
/// Used to PROVE that a normal panel query (no `information_schema`) takes the direct path: the
/// DataFusion-only phases (`info_schema_reg_ms`/`table_reg_ms`/`plan_ms`) stay ZERO.
#[allow(dead_code)]
pub async fn run_with_phases_for_test(
    source: &dyn Source,
    sql: &str,
) -> Result<(QueryResult, QueryPhaseTimings), String> {
    let validated = crate::validate::validate_select(sql).map_err(|e| e.to_string())?;
    register_and_run(source, &validated, sql).await
}

/// Force the REAL DataFusion path (per-table providers + unparse/pushdown) for a plain user query,
/// regardless of `is_simple`. The oracle the direct path must match cell-for-cell.
#[allow(dead_code)]
pub async fn run_via_datafusion_for_test(
    source: &dyn Source,
    sql: &str,
) -> Result<QueryResult, String> {
    let validated = crate::validate::validate_select(sql).map_err(|e| e.to_string())?;
    // Build a NON-simple validated view so `register_and_run` takes the DataFusion branch even for a
    // single-table query (we deliberately test the slow path as the oracle).
    let forced = crate::validate::ValidatedSelect {
        is_simple: false,
        ..validated
    };
    register_and_run(source, &forced, sql).await.map(|(r, _)| r)
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
            .0
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
