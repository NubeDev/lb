//! The `docs.extract` verb — the batch orchestrator (doc-extraction scope). It gates the request
//! once (the capability chokepoint), records a durable job for audit/idempotency, derives each
//! media id in turn (per-item outcomes, each contained by `derive.rs`), then marks the job
//! terminal. Returns `{ job_id, items }` — the job id for status, the per-item outcomes inline
//! (extraction is pure/in-process, so the batch runs to completion in-call; the durable job record
//! + ledger make a re-run idempotent regardless).
//!
//! Why inline rather than a detached `tokio::spawn` (the devkit/federation pattern): v1 extraction
//! is pure CPU with no network and no external — it has nothing to wait on, so a caller watching a
//! job id would only ever see it already `Done`. The job record still exists (audit + a future
//! move to the detached/reactor pattern when OCR/model-assisted extractors add latency), but the
//! honest v1 shape is "run it and return the results".

use lb_auth::Principal;
use lb_jobs::{complete, create, Job, JobStatus};
use lb_store::Store;

use super::authorize::authorize_extract;
use super::derive::derive_one;
use super::error::ExtractSvcError;
use super::model::{ExtractRequest, ItemOutcome};

/// The result of a `docs.extract` run: the durable job id + the per-item outcomes in request order.
#[derive(Debug)]
pub struct ExtractResult {
    pub job_id: String,
    pub items: Vec<ItemOutcome>,
}

/// Run `docs.extract` over `req.media` in `ws` as `principal`. Gates `mcp:docs.extract:call` first
/// (workspace-first); an empty media list is a `BadInput`, not an empty success (a no-op batch is
/// almost always a caller mistake). Derives each item; the job completes even if some items fail.
pub async fn docs_extract(
    store: &Store,
    principal: &Principal,
    ws: &str,
    req: &ExtractRequest,
    ts: u64,
) -> Result<ExtractResult, ExtractSvcError> {
    authorize_extract(principal, ws)?;
    if req.media.is_empty() {
        return Err(ExtractSvcError::BadInput("no media ids to extract".into()));
    }

    // A durable job record (audit + the seam for a future detached/reactor variant). `id` is stable
    // per (caller, ts) so a retried request upserts the same job row rather than spawning a twin.
    let job_id = format!("docs-extract-{}-{ts}", principal.sub());
    let payload = serde_json::json!({ "media": req.media, "tags": req.tags }).to_string();
    let job = Job::new(job_id.clone(), "docs-extract", payload, ts);
    create(store, ws, &job).await?;

    let mut items = Vec::with_capacity(req.media.len());
    for media_id in &req.media {
        items.push(derive_one(store, principal, ws, req, media_id, ts).await);
    }

    // The batch always completes — per-item failures are outcomes, not a failed job. The job is
    // `Failed` only if EVERY item failed for a non-item reason (a store outage), which reads as a
    // genuine batch failure; otherwise `Done`.
    let all_hard_failed = !items.is_empty()
        && items
            .iter()
            .all(|o| matches!(o, ItemOutcome::Failed { .. }));
    let status = if all_hard_failed {
        JobStatus::Failed
    } else {
        JobStatus::Done
    };
    complete(store, ws, &job_id, status).await?;

    Ok(ExtractResult { job_id, items })
}
