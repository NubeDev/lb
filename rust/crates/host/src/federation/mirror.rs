//! `federation.mirror {source, query, target_series, range}` → `{job_id}` — the durable, resumable
//! copy-in path (datasources scope, the `0003` external-warehouse pattern). It enqueues an `lb-jobs`
//! batch that reads the external range and `ingest.write`s it into the platform **series** plane (for
//! dashboards/cache/offline), then resumes mid-range on restart and never double-writes.
//!
//! It reuses `lb-jobs` exactly (it does NOT build a new queue): a `Job` record is the durable
//! checkpoint — its `cursor` is the next external row index to mirror, advanced after each row is
//! committed. A restart re-`run`s with the same `job_id`, loads the cursor, and continues from there.
//! Double-write is impossible regardless: the ingest dedup identity `(series, producer, seq)` makes a
//! re-applied row an upsert of the same slot, never a duplicate.
//!
//! The external read goes through the SAME gated path as `federation.query` (resolve → net:* →
//! mediate DSN → sidecar); the write goes through `ingest_write` (the native host-callback target).

use lb_auth::Principal;
use lb_ingest::Sample;
use lb_jobs::{create, load, Job, JobStatus};
use lb_supervisor::Launcher;
use serde_json::Value;

use super::error::FederationError;
use super::query::federation_query;
use crate::boot::Node;
use crate::ingest::ingest_write;

/// Enqueue + run a mirror of `query` from `source` into `target_series` in `ws`. Returns the durable
/// `job_id`. `range` is the max number of external rows to mirror (a bound; an unbounded export is
/// out of scope). Idempotent + resumable on `job_id`: re-running continues from the checkpoint.
///
/// Each mirrored row maps `rows[i] = [seq, payload]` → a `Sample { series: target_series, seq,
/// payload }`. The first column MUST be an integer seq (the dedup key); the second is the value.
#[allow(clippy::too_many_arguments)]
pub async fn federation_mirror<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    job_id: &str,
    source: &str,
    sql: &str,
    target_series: &str,
    range: usize,
    ts: u64,
) -> Result<String, FederationError> {
    // The query path authorizes `mcp:federation.query:call` (workspace-first) — the mirror reuses it,
    // so a caller without the read cap cannot mirror either (the deny path is shared).
    // Never cached: a mirror is a durable job whose whole purpose is to copy the source's CURRENT
    // rows into the series plane. Serving it a cached answer would persist staleness — exactly the
    // failure mode the result cache's TTL exists to bound in transient page reads only.
    let result = federation_query(node, launcher, caller, ws, source, sql, None, ts).await?;
    let rows = result
        .get("rows")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Load-or-create the durable job; its cursor is the resume point (next row index to mirror).
    let mut job = match load(&node.store, ws, job_id).await? {
        Some(j) => j,
        None => {
            let payload = format!("mirror {source} -> {target_series}");
            let j = Job::new(job_id, "federation-mirror", payload, ts);
            create(&node.store, ws, &j).await?;
            j
        }
    };
    // A completed mirror is a no-op (idempotent re-run).
    if !job.status.is_resumable() {
        return Ok(job_id.to_string());
    }

    let end = rows.len().min(range);
    let start = job.cursor as usize;

    for i in start..end {
        let sample = row_to_sample(target_series, &rows[i])?;
        // ingest_write stamps the producer (the caller) and dedups on (series, producer, seq) — a
        // re-applied row upserts the same slot, so resume never double-writes.
        ingest_write(&node.store, caller, ws, vec![sample])
            .await
            .map_err(|_| FederationError::Denied)?;
        // Commit this row before its checkpoint below, so the cursor only ever advances over rows
        // that really landed. BOUNDED to this row's own batch (drain-backpressure scope): the
        // unbounded drain that used to sit here re-drained the ENTIRE workspace backlog once PER
        // MIRRORED ROW — quadratic in the backlog, the single worst instance of the coupling. One
        // row in, one batch out; the ingest reactor owns the rest.
        crate::ingest::drain_workspace_bounded(&node.store, ws, crate::ingest::own_batches(1))
            .await
            .map_err(|e| FederationError::Sidecar(e.to_string()))?;

        // CHECKPOINT after each committed row: advance the cursor and persist. A crash here resumes
        // from exactly this point (the row just committed is not re-mirrored beyond the dedup).
        job.cursor = (i + 1) as u32;
        job.ts = ts;
        create(&node.store, ws, &job).await?;
    }

    // Mark done only when the ENTIRE available result is mirrored — NOT merely when this call's
    // `range` cap was hit. A partial-range pass leaves the job Running so a later (larger-range) call
    // resumes from the checkpoint; a pass that consumed everything completes it.
    if job.cursor as usize >= rows.len() {
        job.status = JobStatus::Done;
        create(&node.store, ws, &job).await?;
    }

    Ok(job_id.to_string())
}

/// Map an external `[seq, payload]` row to a series [`Sample`]. The seq column is the dedup key.
fn row_to_sample(series: &str, row: &Value) -> Result<Sample, FederationError> {
    let arr = row
        .as_array()
        .ok_or_else(|| FederationError::BadInput("mirror row not an array".into()))?;
    let seq = arr
        .first()
        .and_then(|v| v.as_u64())
        .ok_or_else(|| FederationError::BadInput("mirror row[0] (seq) not an integer".into()))?;
    let payload = arr.get(1).cloned().unwrap_or(Value::Null);
    Ok(Sample {
        series: series.to_string(),
        producer: String::new(), // stamped by ingest_write to the authenticated caller
        ts: seq,
        seq,
        payload,
        labels: Value::Null,
        qos: lb_ingest::Qos::BestEffort,
    })
}
