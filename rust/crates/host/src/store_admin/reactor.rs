//! `spawn_store_compact_reactors` — the background driver that (a) drains enqueued
//! `store-compact` jobs off the request path and (b) logs the threshold advisory when the
//! commit log outgrows [`LOG_ADVISORY_BYTES`](super::status::LOG_ADVISORY_BYTES).
//!
//! **Threshold-driven, never compaction-on-a-tick** (scope: a periodic pass would trade
//! unbounded disk for a periodic I/O storm — the dev-node-cpu lesson). The tick itself is
//! cheap: one indexed pending-jobs query per workspace + one file-metadata stat. A pass runs
//! ONLY when an authorized caller enqueued one; over-threshold logs the advisory and nothing
//! else (scope OQ5: operator-triggered for release 1).
//!
//! Ticks never overlap (`MissedTickBehavior::Skip`); errors are logged, never fatal. The
//! reactor mints no principal — the capability gate ran at `store.compact` enqueue time, the
//! same posture as `spawn_retention_reactors` executing durable retention policy.

use std::sync::Arc;
use std::time::Duration;

use lb_jobs::JobStatus;

use crate::boot::Node;
use crate::store_admin::compact::{CompactJobPayload, STORE_COMPACT_JOB_KIND};
use crate::store_admin::status::{over_threshold_advisory, LOG_ADVISORY_BYTES};

/// The drain/advisory cadence. Fast enough that an operator's enqueue starts promptly; the
/// tick does no heavy work by itself.
pub const STORE_COMPACT_PERIOD: Duration = Duration::from_secs(30);

/// Spawn the detached compact-job drain + log-size advisory for `workspaces`. Returns
/// immediately; the loop runs for the life of the node.
pub fn spawn_store_compact_reactors(node: Arc<Node>, workspaces: Vec<String>, period: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            // The advisory: visible before painful. One stat per tick, warn only while over.
            let snap = lb_store::status(&node.store);
            if let Some(warning) = over_threshold_advisory(snap.log_bytes, LOG_ADVISORY_BYTES) {
                tracing::warn!(log_bytes = snap.log_bytes, "{warning}");
            }
            for ws in &workspaces {
                if let Err(e) = drain_compact_jobs(&node, ws).await {
                    tracing::warn!(ws = %ws, error = %e, "store-compact drain failed");
                }
            }
        }
    });
}

/// Run every pending `store-compact` job in `ws`: execute the pass, record the outcome on the
/// job record, complete it. Sequential — passes must never overlap (each quiesces all writes).
pub async fn drain_compact_jobs(node: &Arc<Node>, ws: &str) -> Result<(), lb_store::StoreError> {
    let pending = lb_jobs::pending(&node.store, ws, STORE_COMPACT_JOB_KIND).await?;
    for mut job in pending {
        let started = std::time::Instant::now();
        let result = lb_store::compact(&node.store).await;
        let mut payload: CompactJobPayload =
            serde_json::from_str(&job.payload).unwrap_or(CompactJobPayload {
                requested_by: String::new(),
                outcome: None,
                error: None,
            });
        let status = match result {
            Ok(rec) => {
                tracing::info!(
                    ws = %ws,
                    job = %job.id,
                    before_bytes = rec.before_bytes,
                    after_bytes = rec.after_bytes,
                    duration_ms = rec.duration_ms,
                    "store compaction pass complete"
                );
                payload.outcome = Some(rec);
                JobStatus::Done
            }
            Err(e) => {
                tracing::warn!(ws = %ws, job = %job.id, error = %e, "store compaction pass failed");
                payload.error = Some(e.to_string());
                JobStatus::Failed
            }
        };
        job.payload = serde_json::to_string(&payload).unwrap_or_default();
        lb_jobs::create(&node.store, ws, &job).await?; // upsert the outcome onto the record
        lb_jobs::complete(&node.store, ws, &job.id, status).await?;
        tracing::debug!(ws = %ws, job = %job.id, elapsed_ms = started.elapsed().as_millis() as u64, "store-compact job drained");
    }
    Ok(())
}
