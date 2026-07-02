//! List the still-running jobs of a given `kind` in a workspace — the reactor's **drain scan**
//! (run-lifecycle #5). A background driver that holds only `Arc<Node>` cannot know which jobs are
//! waiting to be picked up; this is the durable "what is queued" read it ticks on, the same way
//! `flows_list_internal` feeds the flow cron reactor.
//!
//! Only jobs that are still [`JobStatus::is_resumable`] are returned — a `Done`/`Failed`/`Cancelled`
//! job is drained and must never be re-driven (that would double-run, double-spend, double-post). The
//! scan is workspace-namespaced by [`scan`], so a ws-B drain never sees a ws-A job (README §7). Raw
//! verb — the reactor holds its own node-internal authority; no caps gate here (like every jobs verb).
//!
//! **Walks every page, not just the first.** `scan` orders by record id ASCENDING and hard-caps a
//! single page at [`MAX_SCAN_LIMIT`] (200) — a workspace's `jobs` table accumulates rows from every
//! kind forever (flows, agent runs, …), so once it holds more than one page, a single first-page-only
//! read can permanently miss a freshly-enqueued job whose id sorts after the oldest 200 rows (a
//! recurring reactor tick would never see it fall on-page, since older rows never age out ahead of it).
//! Bounded by [`MAX_PENDING_PAGES`] as a self-protection ceiling, not because missing a page is
//! considered acceptable — see the debugging entry this fixes.

use lb_store::{scan, Store, StoreError, MAX_SCAN_LIMIT};
use serde_json::Value;

use super::model::Job;
use super::TABLE;

/// Safety ceiling on pages walked per call (200 * 50 = 10,000 rows) — protects a single reactor tick
/// from an unbounded table scan while comfortably covering any workspace's job-table size in practice.
const MAX_PENDING_PAGES: usize = 50;

/// Return every job in workspace `ws` whose `kind` matches and whose status is still resumable
/// (`Running`/`Suspended`) — the jobs a reactor should pick up. Terminal jobs are filtered out so a
/// drained run is never re-driven. Walks the FULL table (paged by [`MAX_SCAN_LIMIT`], up to
/// [`MAX_PENDING_PAGES`]) rather than only the first page, so a job table larger than one page can
/// never silently hide a pending job from the reactor.
pub async fn pending(store: &Store, ws: &str, kind: &str) -> Result<Vec<Job>, StoreError> {
    let mut out = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..MAX_PENDING_PAGES {
        let page = scan(store, ws, TABLE, MAX_SCAN_LIMIT, after.as_deref()).await?;
        let page_len = page.rows.len();
        for row in page.rows {
            // `scan` returns the whole record; a job row stores its fields directly (no `data`
            // wrapper), but tolerate a wrapped shape defensively (the flow list does the same unwrap).
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
        match page.next {
            Some(cursor) if page_len == MAX_SCAN_LIMIT => after = Some(cursor),
            _ => break, // a short page (or no cursor) is the end of the table
        }
    }
    Ok(out)
}
