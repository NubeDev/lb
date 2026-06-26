//! `react_to_approvals` — the **resolution reactor**: a durable scan that auto-starts the coding job
//! the moment its approval lands `Approved`, closing the loop webhook → triage → approval → JOB →
//! outbox → GitHub with no manual `start_job` step (coding-workflow + outbox scope).
//!
//! Altitude — a **durable scan, not a LIVE-query reactor**: it mirrors `relay_outbox` exactly (a
//! function over a durable set, holding no state), because S6/S7 drive the workflow with durable
//! scans + explicit starts, not ephemeral LIVE queries (§6.2). The scan is the source of truth, so a
//! reactor that restarts simply re-reads `approved` and never misses an approval; the LIVE-query push
//! is the latency optimization layered on later (the same follow-up the relay carries). One pass at
//! logical time `now` starts every owed job; call it again and it is a no-op (idempotent).
//!
//! Idempotency — **re-resolving the same approval starts ONE job.** The reactor derives a
//! deterministic `job_id` from the approval id (`job:{approval_id}`) and **skips an approval whose
//! job already exists** (`lb_jobs::load`). So a second approval (a deferred-then-approved item, or a
//! second pass) finds the job present and does nothing — never a second job, never a second PR.
//! Below that, `start_coding_job`'s `create` upsert and `emit_effect`'s effect-id dedup are a second
//! line of defence, but the existence check is what keeps the *pass* a no-op (and avoids re-streaming
//! "job started" chatter on every scan).
//!
//! Authorization — the reactor runs the workflow under a host **service principal** (the workflow
//! service is the actor, not a human), and `start_coding_job` re-runs its own gate
//! (`mcp:workflow.start_job:call`, workspace-first) for that principal. The hard wall holds: a ws-B
//! reactor pass selects ws-B's namespace for the `approved` scan, the `pr_spec`/`job` reads, and the
//! gate — it can physically only start ws-B jobs (mandatory isolation, §7).

use lb_auth::Principal;
use lb_inbox::approved;
use lb_jobs::load;

use super::error::WorkflowError;
use super::pr_spec::pr_spec;
use super::start_job::{start_coding_job, CodingJob};
use crate::boot::Node;

/// The deterministic job id for an approval — stable, so a re-scan addresses the same job record and
/// the existence check (and the gate's `create` upsert) make the reactor idempotent.
pub fn reactor_job_id(approval_id: &str) -> String {
    format!("job:{approval_id}")
}

/// The outcome of one reactor pass: how many jobs were started, and how many approved items were
/// skipped because their job already existed (the idempotent no-op path).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReactorPass {
    pub started: usize,
    pub already_started: usize,
}

/// Run one reactor pass over workspace `ws` as the service `principal` at logical time `now`: for
/// every `Approved` resolution whose job has not yet started, start the durable coding job (which
/// re-checks the approval gate and routes the PR through the outbox). Returns the pass tally.
///
/// An approved item **without** a recorded `PrSpec` is skipped silently (not every approved inbox
/// item is a coding-job request — only those `request_approval` recorded a spec for). The reactor is
/// thus safe to run over any workspace's resolutions.
pub async fn react_to_approvals(
    node: &Node,
    principal: &Principal,
    ws: &str,
    channel: &str,
    now: u64,
) -> Result<ReactorPass, WorkflowError> {
    let mut pass = ReactorPass::default();
    for resolution in approved(&node.store, ws).await? {
        let approval_id = &resolution.item_id;
        let job_id = reactor_job_id(approval_id);

        // Idempotency: a job already started for this approval → no-op (no second job, no re-stream).
        if load(&node.store, ws, &job_id).await?.is_some() {
            pass.already_started += 1;
            continue;
        }

        // Only approvals that carry a recorded PR spec are coding-job requests. Others are skipped.
        let Some(spec) = pr_spec(&node.store, ws, approval_id).await? else {
            continue;
        };

        start_coding_job(
            node,
            principal,
            ws,
            CodingJob {
                job_id: &job_id,
                approval_id,
                scope_doc: &spec.title,
                channel,
                pr: &spec,
                // The PR's idempotency key is derived from the approval — stable across re-deliveries,
                // so github-target's 422-dedup never opens a second PR for the same approval.
                pr_key: &format!("pr:{approval_id}"),
                ts: now,
            },
        )
        .await?;
        pass.started += 1;
    }
    Ok(pass)
}
