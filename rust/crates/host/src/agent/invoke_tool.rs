//! `agent.invoke` over the **MCP bridge** — the transport a NON-BROWSER caller needs.
//!
//! **The gap this closes.** `agent.invoke` was a fully-built verb with no MCP dispatch arm: the cap
//! (`mcp:agent.invoke:call`) exists, the `tools.catalog` descriptor exists (`agent::descriptor`), the
//! edge→hub routed path exists (`invoke_remote`) — but `call_agent_tool` had no `invoke` match, so a
//! call over `/mcp/call` fell through to `NotFound` and answered an opaque **`403 "no such tool"`**
//! while the caller's token carried the grant. That reads as a permissions bug and is a dispatch bug.
//! It is exactly the "silent no-op" trap `tool_call.rs`'s `host_native_tests` warns about, one layer
//! in: a verb can have caps + catalog rows + a routed path and still 404 for want of a `match` arm.
//!
//! **Why it went unnoticed:** every shipped caller reaches the agent on another transport — the
//! browser posts `POST /agent/invoke`, the palette routes to `postAgent` (see `tools/descriptor.rs`),
//! and the channel worker calls the host fn directly. Nothing that *needs* MCP existed yet.
//!
//! **What needs it:** a **reminder** (`action_kind: "mcp-tool"`). A reminder has no browser and no
//! session — its only way to reach a verb is `Action::McpTool` → the `call_tool` chokepoint. So a
//! scheduled agent run (rubix-ai's `edge-bms-agent-scope.md` duty cycle: a daily engineering review
//! fired by cron) is unbuildable without this arm. Generic seam, not a caller special-case (rule 10):
//! anything reaching the MCP bridge — reminder, extension, routed edge — gets the verb.
//!
//! **This adds no authority.** It is a transport, not a widening: the same `authorize_invoke` gate
//! (workspace-first, then `mcp:agent.invoke:call`) fires inside `invoke_via_runtime`, the run is
//! bounded by `agent ∩ caller` exactly as on the HTTP route, and the outer dispatcher has already
//! checked the same cap before reaching here.

use std::sync::Arc;

use lb_auth::Principal;
use serde_json::{json, Value};

use super::dispatch::invoke_via_runtime;
use super::error::AgentError;
use super::menu::reachable_tools;
use super::model_access::AllowedTool;
use super::Substrate;
use crate::boot::Node;
use lb_mcp::ToolError;

/// `agent.invoke` over MCP. Mirrors `POST /agent/invoke` (`role/gateway/src/routes/agent_invoke.rs`)
/// so the two front doors resolve identically: `runtime = None` → the ONE resolution seam (explicit
/// arg → workspace `agent.config.default_runtime` → registry default), the same persona precedence,
/// and the same `reachable_tools` menu. Returns `{ answer, jobId }` — the HTTP route's `InvokeReply`
/// shape, so a reminder's stored result reads the same as the browser's.
///
/// `job_id` is REQUIRED here, unlike the HTTP route (which derives a stable hash from the goal when a
/// browser omits it). A reminder fires on a schedule with a fixed action payload, so a derived-from-
/// goal id would be the SAME id every firing — and the loop is idempotent on `job_id`, so every run
/// after the first would REPLAY the first run's answer without ever calling the model. Demanding the
/// id makes that the caller's explicit choice (a cron templating a date into it) instead of a silent
/// no-op that looks like a frozen agent.
///
/// `ts` is passed IN (the dispatcher's, already taken at the chokepoint) rather than read here —
/// the clock stays at the boundary, so this path is deterministic under test like the rest of core.
pub async fn call_agent_invoke_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    input: &Value,
    ts: u64,
) -> Result<Value, ToolError> {
    let goal = input
        .get("goal")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ToolError::BadInput("missing string arg: goal".into()))?;

    let job_id = input
        .get("job_id")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            ToolError::BadInput(
                "missing string arg: job_id (the loop is idempotent on it; a scheduled caller must \
                 vary it per firing or every run replays the first answer)"
                    .into(),
            )
        })?;

    // The caller's reachable menu, MINUS `agent.invoke` itself. Without the filter the agent's own
    // tool menu would contain the verb that starts an agent: a run could propose `agent.invoke`, and
    // since the dispatcher applies no depth ceiling to host-native verbs, each nested run would carry
    // its own MAX_STEPS budget — an unbounded fan-out (and a token bill) from one scheduled trigger.
    // Removing it from the MENU, not from the caps, is the right layer: a deliberate nested invoke
    // over a separate MCP call still works, but the model is never handed the recursion.
    let tools: Vec<AllowedTool> = reachable_tools(node, principal, ws)
        .await
        .into_iter()
        .filter(|t| t.name != "agent.invoke")
        .collect();

    let answer = invoke_via_runtime(
        node,
        &node.runtimes(),
        input.get("runtime").and_then(Value::as_str),
        input.get("persona").and_then(Value::as_str),
        principal,
        principal.caps(),
        ws,
        job_id,
        goal,
        Substrate {
            skill: input.get("skill").and_then(Value::as_str),
            doc: input.get("doc").and_then(Value::as_str),
        },
        input.get("context"),
        &tools,
        ts,
    )
    .await
    .map_err(invoke_error)?;

    Ok(json!({ "answer": answer, "jobId": job_id }))
}

/// Map an [`AgentError`] onto the MCP error contract, matching the HTTP route's mapping: a gate
/// refusal stays OPAQUE (`Denied` → the same `403` a capless caller gets, no capability/existence
/// leak), a bad input is reported, a missing session is `NotFound`.
fn invoke_error(e: AgentError) -> ToolError {
    match e {
        AgentError::BadInput(m) => ToolError::BadInput(m),
        AgentError::NotFound => ToolError::NotFound,
        _ => ToolError::Denied,
    }
}
