//! `start_coding_job` — THE GATE (vision §3 steps 6–8, coding-workflow scope). Starts the durable
//! coding job **only if** the approval inbox item resolved `Approved`; otherwise it refuses with
//! [`AwaitingApproval`](WorkflowError::AwaitingApproval) and **no job record is created**. That
//! refusal is the genuine gate — not a cosmetic flag.
//!
//! On approval it: creates the durable `lb-jobs` session (the work survives the approver
//! disconnecting); streams progress to the channel (motion, fire-and-forget §6.2); and routes the
//! external effect (open the PR) through the **transactional outbox** (`emit_effect` — the job step
//! and the effect in one transaction). It does NOT call GitHub directly (the must-deliver class goes
//! through the outbox, never raw pub/sub — the whole point of S6).
//!
//! Authorization: `mcp:workflow.start_job:call`, workspace-first (the deny path), THEN the approval
//! gate. Two independent checks — the capability says "may start a job at all"; the gate says "this
//! particular job's approval landed".

use lb_auth::Principal;
use lb_inbox::{resolution, Decision, Item};
use lb_jobs::{complete, create, Job, JobStatus};
use lb_outbox::Effect;

use super::authorize::authorize_workflow;
use super::effect::emit_effect;
use super::error::WorkflowError;
use super::pr_spec::PrSpec;
use crate::boot::Node;
use crate::channel::post;

/// What starting the job needs and where it reports.
pub struct CodingJob<'a> {
    /// The durable job id (stable — re-starting is idempotent on it).
    pub job_id: &'a str,
    /// The approval inbox item id whose resolution gates this job.
    pub approval_id: &'a str,
    /// The scope doc the job implements (recorded in the job payload).
    pub scope_doc: &'a str,
    /// The channel progress streams to.
    pub channel: &'a str,
    /// The pull request to open — the structured coordinates `github-target` needs (`{repo, head,
    /// base, title, body}`). The producer emits this verbatim as the `create_pr` payload; the
    /// adapter maps it without a shaping step (coding-workflow scope, the producer enrichment).
    pub pr: &'a PrSpec,
    /// The stable idempotency key for the PR effect (the receiver dedups on it).
    pub pr_key: &'a str,
    pub ts: u64,
}

/// Start the coding job in workspace `ws` as `caller` — **iff** its approval is `Approved`. Returns
/// the started job's id. Refuses (`AwaitingApproval`) with no side effects if the approval is
/// missing, deferred, or rejected.
pub async fn start_coding_job(
    node: &Node,
    caller: &Principal,
    ws: &str,
    job: CodingJob<'_>,
) -> Result<String, WorkflowError> {
    authorize_workflow(caller, ws, "start_job")?;

    // THE GATE: the job starts only on an `Approved` resolution. No resolution, or
    // rejected/deferred → refuse, creating nothing.
    match resolution(&node.store, ws, job.approval_id).await? {
        Some(r) if r.decision == Decision::Approved => {}
        _ => return Err(WorkflowError::AwaitingApproval),
    }

    // The durable session — survives the approver disconnecting (S5 jobs). Idempotent on job_id.
    let payload = format!("coding-session for {}", job.scope_doc);
    create(
        &node.store,
        ws,
        &Job::new(job.job_id, "coding-session", payload, job.ts),
    )
    .await?;

    // MOTION: "job started" — fire-and-forget progress to the channel (§6.2).
    stream(
        node,
        caller,
        ws,
        job.channel,
        job.job_id,
        "job started",
        job.ts,
    )
    .await?;

    // MUST-DELIVER: open the PR through the transactional outbox — the job step + the effect in one
    // transaction. The job NEVER calls GitHub directly (outbox scope). The payload is the structured
    // `{repo, head, base, title, body}` shape `github-target` maps — emitted verbatim from the spec,
    // so a real PR can be opened (was `{scope_doc}`, which the adapter could not map).
    let pr = Effect::new(
        format!("{}-pr", job.job_id),
        "github",
        "create_pr",
        job.pr.create_pr_payload(),
        job.pr_key,
        job.ts,
    );
    emit_effect(
        &node.store,
        ws,
        job.job_id,
        0,
        "queued PR through outbox",
        &pr,
    )
    .await?;

    // MOTION: "PR queued" progress.
    stream(
        node,
        caller,
        ws,
        job.channel,
        job.job_id,
        "PR queued via outbox",
        job.ts,
    )
    .await?;

    complete(&node.store, ws, job.job_id, JobStatus::Done).await?;
    Ok(job.job_id.to_string())
}

/// Post one progress message to the channel (motion). A bus failure is non-fatal to the durable job
/// (the record is the truth, §3.3), but we surface it so the caller knows the echo did not land.
async fn stream(
    node: &Node,
    caller: &Principal,
    ws: &str,
    channel: &str,
    job_id: &str,
    note: &str,
    ts: u64,
) -> Result<(), WorkflowError> {
    let item = Item::new(
        format!("{job_id}-{note}"),
        channel,
        caller.sub(),
        format!("[{job_id}] {note}"),
        ts,
    );
    post(node, caller, ws, channel, item)
        .await
        .map(|_| ())
        .map_err(|e| match e {
            crate::channel::ChannelError::Denied => WorkflowError::Denied,
            crate::channel::ChannelError::Store(s) => WorkflowError::Store(s),
            other => WorkflowError::Store(lb_store::StoreError::Decode(other.to_string())),
        })
}
