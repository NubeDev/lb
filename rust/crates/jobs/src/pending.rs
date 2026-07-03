//! List the still-running jobs of a given `kind` in a workspace — the reactor's **drain scan**
//! (run-lifecycle #5). A background driver that holds only `Arc<Node>` cannot know which jobs are
//! waiting to be picked up; this is the durable "what is queued" read it ticks on, the same way
//! `flows_list_internal` feeds the flow cron reactor.
//!
//! Only jobs that are still [`JobStatus::is_resumable`] are returned — a `Done`/`Failed`/`Cancelled`
//! job is drained and must never be re-driven (that would double-run, double-spend, double-post). The
//! query is workspace-namespaced by [`query_ws`], so a ws-B drain never sees a ws-A job (README §7).
//! Raw verb — the reactor holds its own node-internal authority; no caps gate here (like every jobs
//! verb).
//!
//! ## Indexed, not a full walk (the CPU-burn fix)
//!
//! The predicate — `kind` matches AND status is resumable — is pushed **into SurrealDB**, backed by
//! the `(data.kind, data.status)` composite index ([`define_job_index`](crate::define_job_index)),
//! so the cost tracks the number of *pending* jobs, not the size of the `job` table. Earlier this
//! walked every page of the table and filtered `kind`/`status` in Rust; a workspace's `job` table
//! accumulates a terminal row per flow run / agent run forever, so on a long-lived node one drain
//! pass grew past the reactor's tick period and the reactors scanned back-to-back at 100% CPU (see
//! `docs/debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md`). The index makes it a lookup.
//!
//! This is *strictly safer* than the paged walk it replaces on the first-page-only property the walk
//! was written to guarantee: the index returns **every** matching row directly, so a genuinely-
//! pending job whose id sorts late can never fall off an unread page — there are no pages. A
//! [`MAX_PENDING`] ceiling stays as a self-protection backstop (a resumable set is a handful in
//! practice — bounded retention keeps even the terminal rows finite), never approached in normal use.

use lb_store::{Store, StoreError};
use serde_json::Value;

use super::model::Job;
use super::TABLE;

/// The two resumable statuses, as their stored (`kebab-case`, per the [`JobStatus`](crate::JobStatus)
/// serde rename) string values. The query's `IN` list must match the on-disk representation exactly.
const RESUMABLE_STATUSES: [&str; 2] = ["running", "suspended"];

/// Safety ceiling on rows returned by one drain query. The resumable set for a `kind` is a handful in
/// practice (a running run or two, plus any suspended-on-a-decision), and bounded retention keeps the
/// terminal rows finite too — this is a backstop against a pathological workspace, not a paging bound.
const MAX_PENDING: usize = 10_000;

/// Return every job in workspace `ws` whose `kind` matches and whose status is still resumable
/// (`Running`/`Suspended`) — the jobs a reactor should pick up. Terminal jobs are excluded by the
/// query predicate so a drained run is never re-driven. Backed by the `(data.kind, data.status)`
/// index (see [`define_job_index`](crate::define_job_index)), so cost is O(pending), not O(table).
pub async fn pending(store: &Store, ws: &str, kind: &str) -> Result<Vec<Job>, StoreError> {
    // Push the whole predicate into SurrealDB. `data.kind`/`data.status` are the on-disk field paths
    // (the host body is nested under `data` — see `define_job_index`), so this hits the composite
    // index. `SELECT data` returns just the host body, matching the wrapped shape `read`/`list` use.
    let sql = format!(
        "SELECT data FROM type::table($tb) \
         WHERE data.kind = $kind AND data.status IN $resumable \
         LIMIT {MAX_PENDING}"
    );
    let bindings = vec![
        ("tb".into(), Value::String(TABLE.to_string())),
        ("kind".into(), Value::String(kind.to_string())),
        (
            "resumable".into(),
            Value::Array(
                RESUMABLE_STATUSES
                    .iter()
                    .map(|s| Value::String(s.to_string()))
                    .collect(),
            ),
        ),
    ];
    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        // Each row is `{ data: <job body> }`; unwrap the `data` field to the job. A foreign/legacy
        // row that fails to decode is skipped, never fatal to the drain (matches the prior body).
        let inner = match row {
            Value::Object(mut o) => o.remove("data").unwrap_or(Value::Null),
            other => other,
        };
        if let Ok(job) = serde_json::from_value::<Job>(inner) {
            out.push(job);
        }
    }
    Ok(out)
}
