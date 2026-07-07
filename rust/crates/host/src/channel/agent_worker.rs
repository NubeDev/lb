//! The **channel agent worker** (channels-agent scope) — the sibling of [`query_worker`](super::query_worker).
//! Called from [`channel::post`](super::post) when the posted item is a `kind:"agent"` request. As of
//! run-lifecycle #5 it no longer drives the run inline: it **enqueues a durable job** and returns, so
//! `post` finishes at once and the run is detached from the POST connection (it survives the tab
//! closing AND a node restart). The background [`agent_reactor`](crate::agent_reactor) drains the
//! queue and calls [`drive_queued_run`] here, which drives the run through the host-owned runtime seam
//! (external-agent #1, `invoke_via_runtime` → the node's [`RuntimeRegistry`](crate::RuntimeRegistry))
//! and posts the durable `kind:"agent_result"` (or `kind:"agent_error"`) item back into the same
//! channel under a system identity. The whole exchange lives in durable channel history and streams
//! over SSE; while a run executes it publishes `RunEvent`s on the run's bus subject, so a watcher sees
//! it live (agent-run Part 3), identically for an in-house or an external agent.
//!
//! **Runtime is selected, not branched.** `AgentPayload.runtime` flows straight into the registry
//! (absent → the in-house `default`; a profile id → an external `AcpRuntime`, e.g.
//! `open-interpreter-default` → Open Interpreter over Z.AI). Whether an external agent is even
//! *present* is the node's cargo feature + config (it installs the external entries via
//! `Node::install_runtimes`); this worker is identical either way.
//!
//! Security invariants (do not weaken), mirroring the query worker:
//!   - TWO grants, in order: channel `bus:chan/{cid}:pub` (the member already passed it to post the
//!     `agent` item), then `mcp:agent.invoke:call` when the run is driven UNDER THE POSTER'S principal
//!     (`invoke_via_runtime`'s gate 1). The poster's identity + caps are carried on the durable
//!     enqueue record and the reactor reconstructs the poster via `Principal::routed` (the same
//!     co-trust reconstruction the routed-agent hub does). A member without the invoke grant is denied
//!     when the run drives and gets an OPAQUE `agent_error` ("agent not permitted"). Inside the run
//!     every tool the agent tries re-runs `caps::check` under the derived `agent ∩ caller` principal
//!     (no widening).
//!   - The deny path is **opaque**: a missing invoke grant AND a named-but-unknown/ungranted runtime
//!     collapse to the SAME "agent not permitted" — so the poster learns nothing about which runtimes
//!     exist. A genuine run fault is an honest, distinct message.
//!   - **Re-entrancy guard:** only `kind:"agent"` enqueues work. The worker's own
//!     `agent_result`/`agent_error` items parse to other variants and are ignored — an infinite loop
//!     is one absent guard away (tested).
//!   - The worker holds NO durable state; a failure never fails the originating post (which already
//!     durably landed) — the worst case is a follow-up `agent_error` (or nothing).

use std::sync::Arc;
use std::time::Duration;

use lb_auth::Principal;
use lb_inbox::Item;
use lb_jobs::{Job, JobStatus};

use super::agent_job::{ChannelAgentJob, CHANNEL_AGENT_KIND};
use super::payload::{
    agent_error_body, agent_result_body, parse_payload, AgentPayload, ItemPayload,
};
use crate::agent::{invoke_via_runtime, reachable_tools, AgentError, Substrate};
use crate::boot::Node;

/// The system identity the worker posts results/errors under (host answering its own request — no
/// `pub` re-check, like the query worker's `system:query-worker`).
pub(crate) const WORKER_AUTHOR: &str = "system:agent-worker";

/// The opaque deny message. A missing invoke grant AND a named-but-unknown runtime collapse to this,
/// so the poster cannot tell "you lack the grant" from "that runtime doesn't exist".
const OPAQUE_DENY: &str = "agent not permitted";

/// Hard byte cap on the persisted answer (mirrors the query worker's 256 KB posture) so a verbose run
/// can't bloat channel history / the bus frame. The full step-by-step stays in the run stream/job.
const AGENT_MAX_BYTES: usize = 256 * 1024;

/// **Supervision ceiling** (run-lifecycle #5): the wall-clock budget one detached run may consume
/// before it is reaped. A hung/looping run (an external subprocess spinning, an in-house loop that
/// won't settle) is aborted at this ceiling and posts an honest `agent_error` instead of leaving the
/// card spinning forever. Fixed node default (the decided slice posture; per-workspace policy is a
/// deferred open question). Dropping the `invoke_via_runtime` future on timeout tears down the run:
/// for the external `AcpRuntime`, dropping the driver future closes the ACP session and the
/// subprocess's stdio, so the child is reaped rather than left a zombie (Drop is the reaper seam).
pub(crate) const RUN_WALL_CEILING: Duration = Duration::from_secs(15 * 60);

/// The honest (non-opaque) message posted when a run is reaped at the ceiling. Distinct from a
/// capability deny — a timeout is a genuine, reportable run fault, not an authorization signal.
const TIMEOUT_MESSAGE: &str = "agent run exceeded its time limit and was stopped";

/// The message posted as an `agent_error` when a run was STOPPED by a user (agent-dock run controls) —
/// distinct from a fault or a deny; the dock renders it as the terminal "stopped" state.
const STOPPED_MESSAGE: &str = "run stopped";

/// How a driven run actually ended, read from the durable run-job status AFTER the drive returns — the
/// loop returns `Ok` whether it finished, was paused, or was stopped, so the status is the authority.
enum RunLifecycle {
    /// The run finished normally (`Done`/`Failed`) — post the durable answer/error.
    Finished,
    /// The run was PAUSED (`Suspended`, resumable) — post nothing; a `resume_run` continues it.
    Paused,
    /// The run was STOPPED (`Cancelled`, terminal) — post the honest `agent_error`.
    Stopped,
}

/// Classify how the run ended by re-reading its durable status (agent-dock run controls). A missing
/// job or a read hiccup falls back to `Finished` (the pre-controls behavior — post the answer).
async fn run_lifecycle_state(node: &Node, ws: &str, run_job: &str) -> RunLifecycle {
    match lb_jobs::load(&node.store, ws, run_job).await {
        Ok(Some(job)) => match job.status {
            JobStatus::Suspended => RunLifecycle::Paused,
            JobStatus::Cancelled => RunLifecycle::Stopped,
            _ => RunLifecycle::Finished,
        },
        _ => RunLifecycle::Finished,
    }
}

/// If `item` is a `kind:"agent"` request, **enqueue** a durable background run and return; the
/// background reactor drives it later (run-lifecycle #5). Otherwise (chat, a `query*` /
/// `agent_result` / `agent_error` payload, …) do nothing — the re-entrancy guard. Never errors: a
/// failure to enqueue never fails the originating post (which already durably landed).
///
/// Detaching the run from the POST connection is the whole point: `post` returns the instant the
/// enqueue job persists, and the run drives itself under the reactor — so closing the tab or restarting
/// the node no longer cancels or loses the run. The poster's identity + caps are captured onto the
/// durable record so the reactor drives the run under the ASKER's authority, not the reactor's.
pub async fn run_if_agent(node: &Node, poster: &Principal, ws: &str, cid: &str, item: &Item) {
    // RE-ENTRANCY GUARD: only a `kind:"agent"` item triggers work. A result/error item (or plain chat,
    // or a query payload) parses to another variant / None and returns here — never feeds on its output.
    let Some(ItemPayload::Agent(AgentPayload {
        goal,
        runtime,
        persona,
        job,
        context,
        context_items,
    })) = parse_payload(&item.body)
    else {
        return;
    };

    let record = ChannelAgentJob {
        cid: cid.to_string(),
        goal,
        runtime,
        persona,
        run_job: job.clone(),
        // The client-reported page context rides on the durable enqueue record so the reactor fences it
        // into the run's goal exactly as an inline drive would (agent-dock scope). Absent → unchanged.
        context,
        // The gathered-context refs (agent-context-basket scope) — resolved + fenced at drive time,
        // against the durable store, so the run sees exactly what durably lives in this channel.
        context_items,
        // The poster's identity + caps — the reactor reconstructs the poster principal from these
        // (`Principal::routed`) so the run acts with the ASKER's authority, bounded by the asker's
        // grants (`agent ∩ poster`). Co-trust reconstruction, in-process + ws-scoped, exactly as the
        // routed-agent hub already does; never used to widen.
        poster_sub: poster.sub().to_string(),
        poster_caps: poster.caps().to_vec(),
        // The run + its result item are ordered strictly after the request item.
        ts: item.ts.saturating_add(1),
    };

    // Persist the enqueue job durably. Idempotent on `q:<run_job>` (a redelivered request upserts the
    // same job — no double run). A failure to enqueue is swallowed: the request item already landed,
    // and the reactor only drains what durably persisted (the alternative — driving inline on an
    // enqueue failure — would re-tie the run to the POST connection we are deliberately detaching).
    let payload = match serde_json::to_string(&record) {
        Ok(p) => p,
        Err(_) => return,
    };
    let enqueue = Job::new(
        ChannelAgentJob::job_id(&job),
        CHANNEL_AGENT_KIND,
        payload,
        record.ts,
    );
    let _ = lb_jobs::create(&node.store, ws, &enqueue).await;
}

/// Drive one queued channel agent run to completion and post its `agent_result`/`agent_error` back,
/// then mark the enqueue job terminal so it is never re-driven. Called by the background reactor for
/// each pending [`ChannelAgentJob`]. This is the work that used to run inline in `post`; it is
/// unchanged except that it now runs detached under the reactor, under the reconstructed poster.
///
/// **Idempotent:** if the correlated answer item (`a:<run_job>`) already exists, the run already
/// completed on a prior tick (or before a restart) — skip driving it again (no re-run, no re-spend,
/// no double-post) and just mark the enqueue job done.
///
/// **Supervised:** the drive is bounded by `ceiling` — a run that exceeds it is reaped (its future is
/// dropped, tearing down an external subprocess) and posts an honest `agent_error`, so a hung/looping
/// run never leaves the card spinning forever. Production passes [`RUN_WALL_CEILING`]; a test passes a
/// tiny ceiling against a scripted hung runtime.
pub async fn drive_queued_run(
    node: &Arc<Node>,
    ws: &str,
    enqueue_id: &str,
    record: &ChannelAgentJob,
    ceiling: Duration,
) {
    let ChannelAgentJob {
        cid,
        goal,
        runtime,
        persona,
        run_job,
        context,
        context_items,
        poster_sub,
        poster_caps,
        ts,
    } = record;

    // IDEMPOTENCY: a completed run already posted `a:<run_job>` — do not re-drive it. Best-effort; a
    // read hiccup falls through to drive (the run itself is idempotent on `run_job` via the job).
    if answer_already_posted(node, ws, cid, run_job).await {
        finish_enqueue(node, ws, enqueue_id).await;
        return;
    }

    // Reconstruct the poster as the co-trust routed principal — the run acts with the asker's
    // authority, bounded by the asker's grants (identical to `AgentInvokeRequest`'s hub-side rebuild).
    let poster = Principal::routed(poster_sub.clone(), ws.to_string(), poster_caps.clone());
    // The label must reflect the runtime that ACTUALLY runs — resolve it (explicit → workspace default
    // → registry default) so an omitted `runtime` with a stored `open-interpreter-default` reads as
    // that, not the misleading `"default"`. Same seam the run itself uses (no divergence).
    let runtime_label =
        crate::agent::resolve_effective_runtime_id(node, &node.runtimes(), ws, runtime.as_deref())
            .await;

    // Resolve the gathered-context refs (agent-context-basket scope) into the goal the RUN sees —
    // fenced, capped, workspace+channel-scoped, and a `rich_result` ref DEREFERENCED through its
    // source tool under the poster (so an attached snapshot card fences its DATA, not its render
    // envelope). The ORIGINAL goal stays what the durable `agent_result`/`agent_error` echoes (the
    // fence is prompt material, not channel history). An over-cap ref list is a fail-closed, honest
    // `agent_error` (like an oversize page context).
    let goal_for_run = match super::context_items::fence_items_into_goal(
        node,
        &poster,
        ws,
        cid,
        goal,
        context_items,
    )
    .await
    {
        Ok(g) => g,
        Err(e) => {
            let body = agent_error_body(goal, &format!("agent run failed: {e}"));
            let _ = post_worker_item(node, ws, cid, run_job, body, *ts).await;
            finish_enqueue(node, ws, enqueue_id).await;
            return;
        }
    };

    // Tell the run WHICH channel this exchange lives in (channel-widgets slice) — a bare fact, not an
    // instruction: tools that take a `cid` (`channel.post` posting a `rich_result` widget) need the id,
    // and the model cannot otherwise know it. The wall still gates whether posting is permitted.
    let goal_for_run = format!("{goal_for_run}\n\n[conversation channel: {cid}]");

    let outcome = drive_run(
        node,
        &poster,
        ws,
        &goal_for_run,
        runtime.as_deref(),
        persona.as_deref(),
        context.as_ref(),
        run_job,
        *ts,
        ceiling,
    )
    .await;

    // RUN CONTROLS (agent-dock): the loop can return `Ok` because it finished OR because it was
    // PAUSED / STOPPED at a turn boundary (both return the partial answer). Re-read the durable run
    // status to tell them apart — only a genuinely-finished run posts the durable `agent_result`.
    match run_lifecycle_state(node, ws, run_job).await {
        // PAUSED: the run is `Suspended`, resumable. Post NOTHING (no answer of record yet) — a later
        // `resume_run` re-enqueues and continues from the cursor. The enqueue job is still retired
        // below so the reactor doesn't re-drive the paused run; resume re-creates it.
        RunLifecycle::Paused => {}
        // STOPPED: the run is `Cancelled`, terminal. Post a distinct, honest `agent_error` so the dock
        // shows the stopped state (not a spinner, not a normal answer). The partial transcript stays
        // for audit.
        RunLifecycle::Stopped => {
            let body = agent_error_body(goal, STOPPED_MESSAGE);
            let _ = post_worker_item(node, ws, cid, run_job, body, *ts).await;
        }
        // FINISHED: the normal path — post the durable answer (or the loop's own error).
        RunLifecycle::Finished => match outcome {
            Ok(answer) => {
                let (answer, truncated) = cap_answer(answer);
                // CHANNEL-WIDGETS (no-`channel.post` dock path): if the agent's answer carries a
                // fenced ```lb-widget block, split it off — strip the block from the persisted
                // `agent_result` text and post the envelope as a separate `rich_result` item to THIS
                // dock channel (the worker owns the cid; the model never calls `channel.post`). The
                // dock's live-refresh merges the widget item in through the same path a `channel.post`
                // would, rendered by the shipped ResponseView. A present-but-invalid block is left in
                // the answer (no widget lands) — the worker is best-effort, not a second gate.
                let (answer, widget_body) =
                    match super::widget_extract::extract_widget_block(&answer) {
                        Some((stripped, body)) => (stripped, Some(body)),
                        None => (answer, None),
                    };
                let body = agent_result_body(goal, &runtime_label, run_job, &answer, truncated);
                let _ = post_worker_item(node, ws, cid, run_job, body, *ts).await;
                if let Some(body) = widget_body {
                    let _ = post_widget_item(node, ws, cid, run_job, body, *ts + 1).await;
                }
            }
            Err(msg) => {
                let body = agent_error_body(goal, &msg);
                let _ = post_worker_item(node, ws, cid, run_job, body, *ts).await;
            }
        },
    }

    // The run is done (a result or an error item landed) — retire the enqueue job so the reactor's
    // next drain skips it. Terminal even on a post failure: the alternative (leaving it Running) would
    // re-drive a run whose answer we already spent the model on.
    finish_enqueue(node, ws, enqueue_id).await;
}

/// Whether the correlated answer item (`a:<run_job>`) already exists in the channel — the durable
/// signal that this run already completed (idempotency across ticks / a node restart mid-drain).
async fn answer_already_posted(node: &Node, ws: &str, cid: &str, run_job: &str) -> bool {
    let want = ChannelAgentJob::result_item_id(run_job);
    matches!(
        lb_inbox::get(&node.store, ws, cid, &want).await,
        Ok(Some(_))
    )
}

/// Mark the enqueue job `Done` so the reactor's next drain no longer picks it up. Best-effort — a
/// failure just means the next tick re-considers it, and the `answer_already_posted` idempotency guard
/// then short-circuits the re-drive.
async fn finish_enqueue(node: &Node, ws: &str, enqueue_id: &str) {
    let _ = lb_jobs::complete(&node.store, ws, enqueue_id, JobStatus::Done).await;
}

/// Drive the run under the poster's authority via the runtime seam, returning the final answer or an
/// already-shaped error message for an `agent_error` item. The opaque/honest split happens here:
/// a named-but-unknown runtime and a capability deny both collapse to [`OPAQUE_DENY`]; a genuine run
/// fault is honest.
///
/// **Supervision:** the whole run is wrapped in `ceiling`. If it elapses first, the run future is
/// dropped — reaping any external subprocess — and this returns the honest [`TIMEOUT_MESSAGE`] so the
/// caller posts an `agent_error` rather than leaving a stuck card. Terminal outcome is fail-closed:
/// the ceiling (host authority) overrides whatever the run would have eventually reported.
#[allow(clippy::too_many_arguments)]
async fn drive_run(
    node: &Arc<Node>,
    poster: &Principal,
    ws: &str,
    goal: &str,
    runtime: Option<&str>,
    persona: Option<&str>,
    context: Option<&serde_json::Value>,
    job: &str,
    ts: u64,
    ceiling: Duration,
) -> Result<String, String> {
    let registry = node.runtimes();

    // OPAQUE unknown-runtime: a named runtime that isn't registered collapses to the same deny as a
    // missing grant — no runtime-existence leak. (Absent runtime → default, always present.)
    if let Some(id) = runtime {
        if registry.resolve(Some(id)).is_err() {
            return Err(OPAQUE_DENY.to_string());
        }
    }

    // The agent acts with the ASKER'S authority, bounded by the asker's grants: effective principal is
    // `agent_caps ∩ caller`, and we pass the poster's own caps as the agent's — so the run can do
    // exactly what the poster is granted, nothing more. (The invoke gate `mcp:agent.invoke:call` fires
    // inside `invoke_via_runtime` under the poster.)
    let agent_caps = poster.caps().to_vec();

    // Surface the poster's REACHABLE tool menu to the loop (default-agent-wiring #3): the same
    // `tools.catalog` gate that answers the `/`-palette computes "every tool the poster may run" — so
    // the in-house model has real tools to propose, not the empty list this worker used to pass. The
    // wall re-checks every proposed call under `agent ∩ caller`, so the menu is not a widening (a tool
    // absent from the menu is also DENIED if proposed). Best-effort: a catalog read failure (e.g. the
    // poster lacks `mcp:tools.catalog:call`) yields an empty menu — the run still drives (it just has
    // no tools to propose), never fails here.
    let tools = reachable_tools(node, poster, ws).await;

    let run = invoke_via_runtime(
        node,
        &registry,
        runtime,
        persona,
        poster,
        &agent_caps,
        ws,
        job,
        goal,
        Substrate::default(),
        context,
        &tools,
        ts,
    );

    // SUPERVISION: bound the whole run by the wall-clock ceiling. On timeout the `run` future is
    // dropped here — for an external `AcpRuntime` that closes the ACP session + the subprocess stdio,
    // so the child is reaped (Drop is the reaper), not left a zombie pinning the job. The ceiling is
    // authoritative over the run's own eventual outcome (fail-closed).
    match tokio::time::timeout(ceiling, run).await {
        // Ran to completion within the ceiling.
        Ok(result) => result.map_err(|e| match e {
            // Deny is opaque — same message as an unknown runtime (no capability/existence leak).
            AgentError::Denied => OPAQUE_DENY.to_string(),
            // A genuine run fault is honest and distinct (e.g. the external subprocess failed).
            other => format!("agent run failed: {other}"),
        }),
        // Exceeded the ceiling → reaped. Honest, distinct message (not the opaque deny).
        Err(_elapsed) => Err(TIMEOUT_MESSAGE.to_string()),
    }
}

/// Enforce the byte cap on the answer. Returns `(answer, truncated)`; trims to a char boundary at or
/// below [`AGENT_MAX_BYTES`] so the persisted item stays bounded. Pure.
fn cap_answer(answer: String) -> (String, bool) {
    if answer.len() <= AGENT_MAX_BYTES {
        return (answer, false);
    }
    let mut end = AGENT_MAX_BYTES;
    while end > 0 && !answer.is_char_boundary(end) {
        end -= 1;
    }
    (answer[..end].to_string(), true)
}

/// Post a worker result/error item under the system identity via the shared channel `deliver`
/// (STATE-first, then MOTION) — no `pub` gate (the host is posting its own answer). The id ties the
/// answer to the run (`a:<job>`) so a client can correlate them; `ts` orders it after the request.
async fn post_worker_item(
    node: &Node,
    ws: &str,
    cid: &str,
    job: &str,
    body: String,
    ts: u64,
) -> Result<(), super::error::ChannelError> {
    let item = Item::new(format!("a:{job}"), cid, WORKER_AUTHOR, body, ts);
    super::post::deliver(&node.store, &node.bus, ws, cid, item)
        .await
        .map(|_| ())
}

/// Post a worker-authored **widget** item — a `rich_result` render envelope the agent emitted as a
/// fenced block in its answer (the no-`channel.post` dock path). Same `deliver` path as the answer
/// (STATE-first, then MOTION; no `pub` gate — the host posts its own widget), id `w:<job>` so a
/// client can correlate it with the run; `ts` orders it after the `agent_result`.
async fn post_widget_item(
    node: &Node,
    ws: &str,
    cid: &str,
    job: &str,
    body: String,
    ts: u64,
) -> Result<(), super::error::ChannelError> {
    let item = Item::new(format!("w:{job}"), cid, WORKER_AUTHOR, body, ts);
    super::post::deliver(&node.store, &node.bus, ws, cid, item)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_answer_passes_small_answers_through() {
        let (a, t) = cap_answer("hello".to_string());
        assert_eq!(a, "hello");
        assert!(!t);
    }

    #[test]
    fn cap_answer_trims_oversized_at_char_boundary() {
        let big = "é".repeat(AGENT_MAX_BYTES); // 2 bytes each → well over the cap
        let (a, t) = cap_answer(big);
        assert!(t);
        assert!(a.len() <= AGENT_MAX_BYTES);
        // Trimmed at a char boundary — the string is still valid UTF-8 (no panic on slicing).
        assert!(a.chars().all(|c| c == 'é'));
    }
}
