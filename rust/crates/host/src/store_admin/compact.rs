//! `store.compact` — enqueue a compaction pass as a durable job. Whole-log I/O with no upper
//! bound must never run on a request path (drain-backpressure lesson: a request pays for its
//! own work; this job pays for the backlog *explicitly and observably*), so the verb writes a
//! `job:{id}` record and returns; [`spawn_store_compact_reactors`](super::reactor) drains it.

use lb_auth::Principal;
use lb_jobs::Job;
use lb_store::Store;
use serde::{Deserialize, Serialize};

use super::authorize::authorize_store_compact;
use super::error::StoreAdminError;

/// The job kind the reactor drains. One pass per job; the pass is node-wide (one store per
/// node), the job record lives in the enqueuing caller's workspace.
pub const STORE_COMPACT_JOB_KIND: &str = "store-compact";

/// What `store.compact` returns: the job to watch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactEnqueued {
    pub job_id: String,
}

/// The job payload (opaque to `lb_jobs`): who asked, and — once the reactor ran — the outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactJobPayload {
    pub requested_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<lb_store::CompactionRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Enqueue a compaction pass (gated `store:compact:run`). Returns the durable job id; progress
/// and the `{before_bytes, after_bytes}` outcome land on the job record.
pub async fn store_compact_enqueue(
    store: &Store,
    principal: &Principal,
    ws: &str,
    now_ms: u64,
) -> Result<CompactEnqueued, StoreAdminError> {
    authorize_store_compact(principal, ws)?;
    let job_id = format!("store-compact-{}", lb_store::new_ulid());
    let payload = serde_json::to_string(&CompactJobPayload {
        requested_by: principal.sub().to_string(),
        outcome: None,
        error: None,
    })
    .unwrap_or_default();
    let job = Job::new(job_id.clone(), STORE_COMPACT_JOB_KIND, payload, now_ms);
    lb_jobs::create(store, ws, &job).await?;
    Ok(CompactEnqueued { job_id })
}
