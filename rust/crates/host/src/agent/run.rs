//! The tool-call **loop** — the agent itself (agent scope: "the agent owns the loop; the gateway
//! does model access only"). This is where the slice's behavior lives, so it is the one file worth
//! reading top to bottom.
//!
//! The loop, bounded by [`MAX_STEPS`] (no runaway / budget burn — the ceiling is the agent's, not
//! the gateway's):
//!   1. ask the model for a turn (`ModelAccess::turn`) — replay-safe by a per-step idempotency key;
//!   2. for each proposed tool call, run it through `lb_mcp::call` under the **derived** principal
//!      (capability-checked, workspace-first, routed if remote). A denial is fed back as a tool
//!      error, NOT a crash — the model can react;
//!   3. persist the step to the job (idempotent, append-addressed) and advance the cursor;
//!   4. repeat until the model is `done` or the ceiling is hit; then `complete` the job.
//!
//! Resume re-enters at the job's `cursor`: steps already persisted are NOT re-run (their model turn
//! is cached by the gateway's idempotency key anyway), so a session that survived an edge
//! disconnect continues without double-applying or re-spending (agent scope offline/sync).

use lb_auth::Principal;
use lb_jobs::{append_step, complete, create, load, Job, JobStatus};
use lb_mcp::call;

use super::error::AgentError;
use super::model_access::{AllowedTool, CallOutcome, ModelAccess, ProposedCall};
use crate::boot::Node;

/// The loop ceiling. A fixed default at S5 (a per-workspace policy is a scope follow-up).
pub const MAX_STEPS: u32 = 8;

/// The derived-actor sub prefix — audit shows `agent:{skill-or-goal}` acted on the caller's behalf.
const AGENT_SUB: &str = "agent:session";

/// Run (or resume) an agent session to completion. `agent_caps` are the agent's own capabilities;
/// the effective grant is `agent_caps ∩ caller.caps` via the derived principal (no widening). The
/// session is the durable job `job_id`; on resume it continues from the persisted cursor.
///
/// `tools` are the qualified MCP tool names the model may propose. The loop returns the final
/// model content (the session's answer). Errors only on a gate refusal at the surface or a store
/// failure — a tool denial *inside* the loop is fed to the model, not surfaced as an error.
#[allow(clippy::too_many_arguments)]
pub async fn run_session<M: ModelAccess>(
    node: &Node,
    model: &M,
    caller: &Principal,
    agent_caps: &[String],
    ws: &str,
    job_id: &str,
    goal: &str,
    tools: &[AllowedTool],
    ts: u64,
) -> Result<String, AgentError> {
    // The derived (intersected) principal: the agent acts under `agent_caps ∩ caller.caps`, same ws,
    // under a distinct `agent:*` sub (audit shows the agent acted). It inherits exactly what BOTH
    // sides allow — never more (agent scope no-widening). Tool calls are workspace + capability
    // gated only (no membership), so the distinct sub is correct here (substrate reads, which ARE
    // membership-gated, use the caller's identity — see `substrate.rs`).
    let agent = caller.derive(AGENT_SUB, agent_caps.to_vec());

    // Create the session if new; resume the existing record otherwise (idempotent on job_id).
    let mut job = match load(&node.store, ws, job_id).await? {
        Some(existing) => existing,
        None => {
            let job = Job::new(job_id, "agent-session", goal, ts);
            create(&node.store, ws, &job).await?;
            job
        }
    };

    let mut messages: Vec<(String, String)> = vec![
        ("system".into(), "You are a workspace agent.".into()),
        ("user".into(), goal.to_string()),
    ];
    let mut prior: Vec<CallOutcome> = Vec::new();
    let mut last_content = String::new();

    // Continue from the durable cursor — steps before it already landed (resume idempotency).
    let mut step = job.cursor;
    while step < MAX_STEPS {
        // Replay-safe: the gateway caches by this key, so a resumed turn does not re-spend.
        let key = format!("{ws}:{job_id}:{step}");
        let turn = model.turn(ws, &messages, tools, &prior, &key).await;
        last_content = turn.content.clone();
        if !turn.content.is_empty() {
            messages.push(("assistant".into(), turn.content.clone()));
        }

        if turn.done || turn.calls.is_empty() {
            break;
        }

        // Run each proposed call under the DERIVED principal — capability-checked, workspace-first,
        // routed if the tool lives on another node. A denial is fed back, not raised.
        prior = run_calls(node, &agent, ws, &turn.calls).await;
        let summary = summarize(&prior);

        // Persist the step (idempotent, append-addressed) and advance the cursor — durable BEFORE
        // the next turn, so an edge disconnect here leaves a resumable record.
        append_step(&node.store, ws, job_id, step, &summary).await?;
        job.cursor = step + 1;

        messages.push(("tool".into(), summary));
        step += 1;
    }

    complete(&node.store, ws, job_id, JobStatus::Done).await?;
    Ok(last_content)
}

/// Run each proposed tool call under the derived principal, collecting outcomes. `lb_mcp::call`
/// runs the SAME `caps::check` chokepoint — so an agent call to a tool the intersection forbids is
/// `Denied`, captured as an error outcome (the model is told; the loop continues).
async fn run_calls(
    node: &Node,
    agent: &Principal,
    ws: &str,
    calls: &[ProposedCall],
) -> Vec<CallOutcome> {
    let mut outcomes = Vec::with_capacity(calls.len());
    for c in calls {
        let outcome = match call(&node.registry, &node.bus, agent, ws, &c.name, &c.input).await {
            Ok(out) => CallOutcome {
                id: c.id.clone(),
                ok: Some(out),
                error: None,
            },
            Err(e) => CallOutcome {
                id: c.id.clone(),
                ok: None,
                error: Some(e.to_string()),
            },
        };
        outcomes.push(outcome);
    }
    outcomes
}

/// A compact, durable summary of a turn's tool outcomes — what lands in the job transcript.
fn summarize(outcomes: &[CallOutcome]) -> String {
    let parts: Vec<String> = outcomes
        .iter()
        .map(|o| match (&o.ok, &o.error) {
            (Some(ok), _) => format!("{}=ok:{ok}", o.id),
            (_, Some(err)) => format!("{}=err:{err}", o.id),
            _ => format!("{}=empty", o.id),
        })
        .collect();
    parts.join("; ")
}
