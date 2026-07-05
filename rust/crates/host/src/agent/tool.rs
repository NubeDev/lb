//! The host-native `agent.*` MCP verb dispatcher (agent-run scope Part 2 — policy + decision).
//!
//! These are host verbs over the embedded store (not runtime-registry components), reached through the
//! one MCP contract from `tool_call.rs`'s `agent.` branch. Each runs the standard MCP gate
//! (workspace-first, then `mcp:<verb>:call`, opaque `Denied`) before touching the store:
//!   - `agent.policy.set`  — write the ws permission policy. Gated by an **admin** cap
//!     (`mcp:agent.policy.set:call`) — editing who-may-run-what is an admin act, not a member one.
//!   - `agent.decide`      — first-settle a suspended tool call. Gated by `mcp:agent.decide:call` —
//!     the same authority that resolves the surfaced inbox item, but the binding write is the
//!     `agent_decision` record (first-settle), not the last-writer-wins inbox row.
//!   - `agent.runtimes`    — the run-lifecycle #5 read surface: list the node's configured runtimes
//!     for the composer runtime picker. Gated by `mcp:agent.runtimes:call` (a distinct read cap);
//!     read-only, list-only, registry-derived (see `runtimes.rs`).
//!
//! `agent.watch` (Part 3) is **not** a JSON-returning verb here: it yields a live `RunEvent` *stream*,
//! so — exactly like `bus.watch`/`channel.stream` — its transport is the gateway **SSE route**, which
//! calls `lb_host::watch_run` directly (the `mcp:agent.watch:call` cap is checked inside `watch_run`).
//! A request-shaped JSON `agent.watch` call therefore has no meaningful single value to return and is
//! left as `NotFound` on this dispatch path; the live feed lives on the SSE route.
//!
//! Single responsibility: this file is *dispatch + arg parsing + the verb gate*; the durable logic
//! lives in `policy/` and `decision/`.

use lb_auth::Principal;
use lb_jobs::SuspensionDecision;
use lb_mcp::{authorize_tool, ToolError};
use serde_json::{json, Value};

use super::decision::{settle_decision, SettleOutcome};
use super::policy::{save_policy, Policy};
use super::runtimes::list_runtimes;
use crate::boot::Node;

/// Dispatch a qualified `agent.*` verb. The single entry `tool_call.rs` delegates to; it matches the
/// verb and routes to the policy/decision handlers (watch is Part 3).
pub async fn call_agent_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "agent.policy.set" => call_agent_policy_tool(node, principal, ws, input).await,
        // agent-personas #1 Settings surface: read the ws permission policy so the Allow/Ask/Deny pane
        // can render current rules (the policy `set` verb replaces the whole list, so the pane needs a
        // read to round-trip). Member-level (`mcp:agent.policy.get:call`) — reading the policy that
        // governs your runs is not an admin act; editing it (`agent.policy.set`) stays admin.
        "agent.policy.get" => call_agent_policy_get(node, principal, ws).await,
        "agent.decide" => call_agent_decision_tool(node, principal, ws, input).await,
        // agent.runtimes (run-lifecycle #5 read surface): list the node's configured runtimes for the
        // composer runtime picker. Read-only, list-only; its own read cap gates it inside.
        "agent.runtimes" => list_runtimes(node, principal, ws).await,
        // agent-config scope: the per-workspace default-runtime + model-endpoint record.
        // `agent.config.get` (member) / `agent.config.set` (admin). `call_agent_config_tool` returns
        // `None` for a verb outside its surface, so it composes as a fall-through before `NotFound`.
        "agent.config.get" | "agent.config.set" => {
            match super::call_agent_config_tool(node, principal, ws, qualified_tool, input).await {
                Some(r) => r,
                None => Err(ToolError::NotFound),
            }
        }
        // agent-catalog scope: the definition catalog verbs (`agent.def.list|get|create|update|
        // delete`). Each runs its own MCP gate + reserved-tier + runtime validation inside; the
        // dispatcher returns `None` for a verb outside its surface (fall-through before `NotFound`).
        _ if qualified_tool.starts_with("agent.def.") => {
            match super::call_agent_catalog_tool(node, principal, ws, qualified_tool, input).await {
                Some(r) => r,
                None => Err(ToolError::NotFound),
            }
        }
        // agent-personas scope #1: the persona catalog verbs (`agent.persona.list|get|create|update|
        // delete`). Each runs its own MCP gate + reserved-tier + field validation inside; the
        // dispatcher returns `None` for a verb outside its surface (fall-through before `NotFound`).
        _ if qualified_tool.starts_with("agent.persona.") => {
            match super::call_agent_persona_tool(node, principal, ws, qualified_tool, input).await {
                Some(r) => r,
                None => Err(ToolError::NotFound),
            }
        }
        // agent-memory scope: the durable memory verbs (`agent.memory.list|get|set|delete`). Each
        // runs its own MCP + member-wall + ws-write gate inside; the dispatcher returns `None` for a
        // verb outside its surface, so it composes as a fall-through before `NotFound`.
        _ if qualified_tool.starts_with("agent.memory.") => {
            match super::call_agent_memory_tool(&node.store, principal, ws, qualified_tool, input)
                .await
            {
                Some(r) => r,
                None => Err(ToolError::NotFound),
            }
        }
        // agent.watch is added by Part 3 (the start/resume-vs-watch split). Left explicit so the two
        // parts share the `agent.` prefix without one swallowing the other's verbs.
        _ => Err(ToolError::NotFound),
    }
}

/// `agent.policy.get` (member) — the ws permission policy (`{ rules: [...] }`; an empty list when
/// none is set = default-allow). Read-only: the Allow/Ask/Deny Settings pane reads it to render the
/// current rules before an admin edits + `set`s them back. Gated by `mcp:agent.policy.get:call`.
async fn call_agent_policy_get(
    node: &Node,
    principal: &Principal,
    ws: &str,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, "agent.policy.get").map_err(|_| ToolError::Denied)?;
    let policy = crate::agent::load_policy(&node.store, ws)
        .await
        .map_err(|_| ToolError::Denied)?;
    serde_json::to_value(&policy).map_err(|e| ToolError::BadInput(e.to_string()))
}

/// `agent.policy.set {rules: [...]}` — replace the workspace permission policy. Admin-gated. The whole
/// rule list is replaced (last-writer-wins config; the first-settle guarantee is on the decision
/// record, not here).
async fn call_agent_policy_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, "agent.policy.set").map_err(|_| ToolError::Denied)?;
    // The input is the policy record itself (`{"rules": [...]}`); decode it straight into `Policy` so
    // the rule/effect grammar is the single source of truth (no parallel parse).
    let policy: Policy = serde_json::from_value(input.clone())
        .map_err(|e| ToolError::BadInput(format!("policy: {e}")))?;
    save_policy(&node.store, ws, &policy)
        .await
        .map_err(|_| ToolError::Denied)?;
    Ok(json!({ "ok": true, "rules": policy.rules.len() }))
}

/// `agent.decide {job_id, tool_call_id, decision}` — first-settle a suspended tool call. Gated by
/// `mcp:agent.decide:call`. Returns whether THIS call bound the decision (`bound: true`) or it was
/// already settled (`bound: false`, the idempotent duplicate / re-scan no-op).
async fn call_agent_decision_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, "agent.decide").map_err(|_| ToolError::Denied)?;
    let job_id = input
        .get("job_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput("missing arg: job_id".into()))?;
    let tool_call_id = input
        .get("tool_call_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput("missing arg: tool_call_id".into()))?;
    let decision: SuspensionDecision =
        serde_json::from_value(input.get("decision").cloned().unwrap_or(Value::Null))
            .map_err(|e| ToolError::BadInput(format!("decision: {e}")))?;
    let ts = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);

    let outcome = settle_decision(&node.store, ws, job_id, tool_call_id, decision, ts)
        .await
        // A settle for an unopened call is a Conflict from the store — surface it as a BadInput-style
        // refusal (there is nothing to decide), not an opaque Denied (the caller WAS authorized).
        .map_err(|_| ToolError::BadInput("no pending decision for that tool call".into()))?;
    match outcome {
        SettleOutcome::Bound(_) => Ok(json!({ "ok": true, "bound": true })),
        SettleOutcome::AlreadySettled(_) => Ok(json!({ "ok": true, "bound": false })),
    }
}
