//! The `agent_invoke` command — the shell's IPC verb the UI's `agent.api.ts::invokeAgent` calls
//! (active-agent-wiring Slice 5, the dashboard "AI widget"). Thin glue over `lb_host`, the desktop
//! peer of the gateway's `POST /agent/invoke`: it drives the workspace's ACTIVE agent (NO runtime →
//! the run seam resolves the workspace default) and returns the durable job id + final answer. The
//! self-gate (`mcp:agent.invoke:call`, workspace-first) fires inside `invoke_via_runtime` — the shell
//! adds no authority of its own; the session principal is the wall.

use lb_host::{invoke_via_runtime, reachable_tools, Substrate};
use serde::Serialize;

use crate::state::NodeHandle;

/// The run's result — the UI's `AgentResult` shape (`agent.types.ts`): the final answer + the durable
/// job/session id. `job_id` is renamed to the `jobId` the client reads.
#[derive(Debug, Serialize)]
pub struct AgentResult {
    pub answer: String,
    #[serde(rename = "jobId")]
    pub job_id: String,
}

/// Invoke the workspace's active agent for `goal` as the session principal (optionally over a granted
/// `skill` / shared `doc`). `job_id` correlates the run; absent, a stable id is derived from
/// `(ws, goal)`. Errors are stringified for the IPC boundary (a `Denied` reads as "denied").
pub async fn agent_invoke(
    handle: &NodeHandle,
    goal: &str,
    job_id: Option<&str>,
    skill: Option<&str>,
    doc: Option<&str>,
    ts: u64,
) -> Result<AgentResult, String> {
    let ws = &handle.ws;
    let job_id = job_id
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| stable_job_id(ws, goal));

    // The caller's reachable tool menu (default-agent-wiring #3) — the same `tools.catalog` gate the
    // `/`-palette reads. Best-effort: a read failure yields an empty menu, never an error here.
    let tools = reachable_tools(&handle.node, &handle.principal, ws).await;

    let answer = invoke_via_runtime(
        &handle.node,
        &handle.node.runtimes(),
        None, // no runtime → the run seam resolves the workspace's active pick
        &handle.principal,
        &handle.principal.caps().to_vec(),
        ws,
        &job_id,
        goal,
        Substrate { skill, doc },
        &tools,
        ts,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(AgentResult { answer, job_id })
}

/// A deterministic job id from `(ws, goal)` — the idempotent fallback when the caller supplies none
/// (mirrors the gateway route; a stable hash, never wall-clock/rng, so the loop is idempotent).
fn stable_job_id(ws: &str, goal: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    ws.hash(&mut h);
    goal.hash(&mut h);
    format!("agent-{:016x}", h.finish())
}
