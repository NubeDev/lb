//! Coding-workflow routes — the browser's `workflow.*` surface over the gateway (S6 follow-up: the
//! host had the orchestration verbs + the `workflow.*` MCP bridge, but only the Tauri shell reached
//! them, so a browser threw `unknown command`). Each route mirrors a `lb_host::<verb>` 1:1 and
//! re-runs the host's own gates server-side (workspace-first, then `mcp:workflow.<verb>:call`, then
//! the verb's own gate — the headline being the S6 **approval gate**: a job starts ONLY on an
//! `Approved` resolution). The workspace + the principal come from the **token**, never the body
//! (the hard wall, §7).
//!
//! The approval id is a path segment; the PR coordinates are recorded once at `request` time and
//! read back by `start` (the same contract the `workflow.*` MCP bridge's `start_job` uses — no
//! redundant PR args on the start wire). Reading the workspace's outbox (the UI's "PR queued →
//! delivered" view) is the existing `GET /outbox` route — not re-exposed here.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{CodingJob, PrSpec, WorkflowError};
use lb_inbox::Decision;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `POST /approvals/{id}/request` body — the scope doc, the reviewing team, and the PR coordinates
/// to open once the job runs. The PR spec is persisted keyed by the approval id (read back by
/// `start`), so it is supplied exactly once, here.
#[derive(Debug, Deserialize)]
pub struct RequestApproval {
    pub scope_doc: String,
    pub team: String,
    pub pr: PrSpec,
}

/// `POST /approvals/{id}/request` — open a `needs:approval` inbox item gating a coding job, and
/// record its PR spec. Returns `{ id }` (the inbox item id).
pub async fn request_approval(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<RequestApproval>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let item = lb_host::request_approval(
        &gw.node.store,
        &p,
        p.ws(),
        &id,
        &body.scope_doc,
        &body.team,
        &body.pr,
        gw.now,
    )
    .await
    .map_err(wf_status)?;
    Ok(Json(json!({ "id": item.id })))
}

/// `POST /approvals/{id}/resolve` body — the reviewer's decision.
#[derive(Debug, Deserialize)]
pub struct ResolveApproval {
    pub decision: Decision,
}

/// `POST /approvals/{id}/resolve` — record approve/reject/defer on the approval item (the S6 gate
/// the `start` route reads). Returns `{ ok: true }`.
pub async fn resolve_approval(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ResolveApproval>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    lb_host::resolve_approval(&gw.node.store, &p, p.ws(), &id, body.decision, gw.now)
        .await
        .map_err(wf_status)?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /approvals/{id}/start` body — the durable job id, the scope doc it implements, the channel
/// progress streams to, and the PR effect's idempotency key. The PR coordinates are NOT here — they
/// were recorded at `request` time and are read back by approval id (mirrors the MCP bridge).
#[derive(Debug, Deserialize)]
pub struct StartJob {
    pub job_id: String,
    pub scope_doc: String,
    pub channel: String,
    pub pr_key: String,
}

/// `POST /approvals/{id}/start` — start the gated coding job. Succeeds (`{ job_id, started: true }`)
/// **iff** the approval resolved `Approved`; the genuine gate refuses (`{ started: false }`) with no
/// side effects if it is missing/deferred/rejected. Idempotent on the job id.
pub async fn start_job(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<StartJob>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    // The PR coordinates were persisted at `request` time, keyed by approval_id — read them back
    // (the same contract as the MCP bridge's `start_job`). A missing spec means this approval was
    // never a coding-job request → a `400`, distinct from the approval gate's `started: false`.
    let spec = lb_host::pr_spec(&gw.node.store, p.ws(), &id)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "no PR spec for approval".to_string(),
            )
        })?;
    match lb_host::start_coding_job(
        &gw.node,
        &p,
        p.ws(),
        CodingJob {
            job_id: &body.job_id,
            approval_id: &id,
            scope_doc: &body.scope_doc,
            channel: &body.channel,
            pr: &spec,
            pr_key: &body.pr_key,
            ts: gw.now,
        },
    )
    .await
    {
        Ok(job_id) => Ok(Json(json!({ "jobId": job_id, "started": true }))),
        // The approval gate refusing is the genuine S6 behaviour — NOT an error to the UI. Surface
        // it as `started: false` (same shape the fake returns), so the UI shows "awaiting approval".
        Err(WorkflowError::AwaitingApproval) => {
            Ok(Json(json!({ "jobId": body.job_id, "started": false })))
        }
        Err(e) => Err(wf_status(e)),
    }
}

/// Map a workflow-gate outcome onto an HTTP status. `Denied` stays `403` (opaque); `NotFound` is
/// `404` (only after the gates pass); `AwaitingApproval` reaching here (i.e. not the `start` route's
/// handled case) is a `400` client error; any inner fault is `403`-opaque like the other routes.
fn wf_status(e: WorkflowError) -> (StatusCode, String) {
    match e {
        WorkflowError::NotFound => (StatusCode::NOT_FOUND, e.to_string()),
        WorkflowError::AwaitingApproval => (StatusCode::BAD_REQUEST, e.to_string()),
        other => (StatusCode::FORBIDDEN, other.to_string()),
    }
}
