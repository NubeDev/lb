//! The public agent entry — `invoke` (and its `resume` sibling). Composes the gates in order, then
//! drives the loop (agent scope example flow):
//!   1. **invoke gate** — `mcp:agent.invoke:call`, workspace-first, on the calling node;
//!   2. **substrate** — load the granted skill + read the shared doc under the DERIVED principal
//!      (capability- + membership/grant-checked, never bypassing the S4 gates);
//!   3. **loop** — `run_session` drives the bounded tool-call loop over a durable job.
//!
//! `invoke` creates the session; `resume` continues an existing one (same `run_session`, which
//! re-enters at the persisted cursor) — the offline/sync path (the edge disconnected; the hub
//! resumes). Both are idempotent on `job_id`.

use lb_auth::Principal;

use super::authorize::authorize_invoke;
use super::error::AgentError;
use super::model_access::{AllowedTool, ModelAccess};
use super::run::run_session;
use super::substrate::{load_substrate_skill, read_substrate_doc};
use crate::boot::Node;

/// What the caller asks the agent to do. `skill`/`doc` are optional substrate references the agent
/// loads under its derived principal before running; `tools` are the qualified MCP tools the model
/// may propose during the loop.
pub struct Invocation<'a> {
    pub job_id: &'a str,
    pub goal: &'a str,
    pub skill: Option<&'a str>,
    pub doc: Option<&'a str>,
    pub tools: &'a [AllowedTool],
    pub ts: u64,
}

/// Invoke the central agent in workspace `ws` for `caller`. Runs the invoke gate, loads any
/// substrate, then drives the loop to completion — returning the agent's final answer. `agent_caps`
/// are the agent actor's own capabilities; the effective grant is `agent_caps ∩ caller.caps`.
pub async fn invoke<M: ModelAccess>(
    node: &Node,
    model: &M,
    caller: &Principal,
    agent_caps: &[String],
    ws: &str,
    inv: Invocation<'_>,
) -> Result<String, AgentError> {
    // Gate 1: may this caller invoke the agent at all? (workspace-first, then mcp:agent.invoke:call)
    authorize_invoke(caller, ws)?;

    // Substrate, read on the caller's behalf — the S4 three gates fire as for the caller
    // (membership/ownership/grant), capabilities bounded to agent ∩ caller (no widening). See
    // `substrate.rs` for why the caller's identity, not `agent:session`, resolves gate 3.
    let mut goal = inv.goal.to_string();
    if let Some(skill_id) = inv.skill {
        let body = load_substrate_skill(&node.store, caller, agent_caps, ws, skill_id).await?;
        goal = format!("{goal}\n\n[skill {skill_id}]\n{body}");
    }
    if let Some(doc_id) = inv.doc {
        let content = read_substrate_doc(&node.store, caller, agent_caps, ws, doc_id).await?;
        goal = format!("{goal}\n\n[doc {doc_id}]\n{content}");
    }

    // The loop derives its own principal from `caller` ∩ `agent_caps` and drives the job.
    run_session(
        node, model, caller, agent_caps, ws, inv.job_id, &goal, inv.tools, inv.ts,
    )
    .await
}

/// Resume a session that may have been interrupted (the edge disconnected mid-loop). Re-runs the
/// loop, which continues from the persisted cursor — idempotent, no double-apply, no re-spend
/// (agent scope offline/sync). The substrate is NOT re-seeded (the goal is already in the record);
/// the loop picks up where the durable cursor left off.
#[allow(clippy::too_many_arguments)]
pub async fn resume<M: ModelAccess>(
    node: &Node,
    model: &M,
    caller: &Principal,
    agent_caps: &[String],
    ws: &str,
    job_id: &str,
    tools: &[AllowedTool],
    ts: u64,
) -> Result<String, AgentError> {
    authorize_invoke(caller, ws)?;
    // The goal (with its substrate already baked in at invoke time) is reloaded from the durable
    // job; the loop continues from the persisted cursor — no re-seed, no double-apply.
    let goal = match lb_jobs::load(&node.store, ws, job_id).await? {
        Some(job) => job.payload,
        None => return Err(AgentError::NotFound),
    };
    run_session(
        node, model, caller, agent_caps, ws, job_id, &goal, tools, ts,
    )
    .await
}
