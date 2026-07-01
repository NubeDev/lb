//! List the still-running jobs of a given `kind` in a workspace — the reactor's **drain scan**
//! (run-lifecycle #5). A background driver that holds only `Arc<Node>` cannot know which jobs are
//! waiting to be picked up; this is the durable "what is queued" read it ticks on, the same way
//! `flows_list_internal` feeds the flow cron reactor.
//!
//! Only jobs that are still [`JobStatus::is_resumable`] are returned — a `Done`/`Failed`/`Cancelled`
//! job is drained and must never be re-driven (that would double-run, double-spend, double-post). The
//! scan is workspace-namespaced by [`scan`], so a ws-B drain never sees a ws-A job (README §7). Raw
//! verb — the reactor holds its own node-internal authority; no caps gate here (like every jobs verb).

use lb_store::{scan, Store, StoreError, MAX_SCAN_LIMIT};
use serde_json::Value;

use super::model::Job;
use super::TABLE;

/// Return every job in workspace `ws` whose `kind` matches and whose status is still resumable
/// (`Running`/`Suspended`) — the jobs a reactor should pick up. Terminal jobs are filtered out so a
/// drained run is never re-driven. Bounded to one page ([`MAX_SCAN_LIMIT`]); a reactor ticks
/// repeatedly, so an overflowing backlog is drained across ticks rather than in one unbounded read.
pub async fn pending(store: &Store, ws: &str, kind: &str) -> Result<Vec<Job>, StoreError> {
    let page = scan(store, ws, TABLE, MAX_SCAN_LIMIT, None).await?;
    let mut out = Vec::new();
    for row in page.rows {
        // `scan` returns the whole record; a job row stores its fields directly (no `data` wrapper),
        // but tolerate a wrapped shape defensively (the flow list does the same unwrap).
        let inner = match row.data {
            Value::Object(mut o) if o.contains_key("data") => {
                o.remove("data").unwrap_or(Value::Null)
            }
            other => other,
        };
        let Ok(job) = serde_json::from_value::<Job>(inner) else {
            continue; // a foreign/legacy row in the job table is skipped, never fatal to the drain
        };
        if job.kind == kind && job.status.is_resumable() {
            out.push(job);
        }
    }
    Ok(out)
}
