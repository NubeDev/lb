//! The MCP bridge for workflow verbs — host-native tools under the one MCP contract (README §6.5,
//! §3.7). The UI and other extensions reach the workflow the SAME way they reach any tool: a
//! qualified `workflow.<verb>` call with JSON in/out.
//!
//! Two gates, in order, like every other tool call:
//!   1. the **MCP gate** — `authorize_workflow` (workspace-first, then `mcp:workflow.<verb>:call`).
//!      This is what makes the mandatory MCP-surface isolation + deny tests real: a ws-B caller (or
//!      one without the grant) is refused HERE, before the verb runs.
//!   2. the **verb gate** — `start_job` then re-checks the approval gate; `resolve_approval` writes
//!      under the caller's audited sub. Independent of the MCP grant.
//!
//! `triage` is NOT bridged here: it drives the AI agent (it needs a `ModelAccess`), so it has its
//! own typed entry (`workflow::triage`) — exactly like the agent's `invoke` is not in this bridge.
//! The bridged verbs are the store/orchestration ones the UI drives directly.

use lb_inbox::Decision;
use lb_mcp::{authorize_tool, ToolError};
use serde_json::{json, Value};

use super::{
    ingest_issue, request_approval, resolve_approval, start_coding_job, CodingJob, WorkflowError,
};
use crate::boot::Node;
use lb_auth::Principal;

/// Dispatch a `workflow.<verb>` MCP call. `input` is the verb's JSON arguments; the return is the
/// verb's JSON result. The MCP gate runs first (via the verb's own `authorize_workflow`).
pub async fn call_workflow_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    // Gate 1: the MCP surface — workspace-first, then mcp:workflow.<verb>:call. Opaque on denial.
    authorize_tool(principal, ws, qualified_tool)?;

    let verb = qualified_tool
        .split_once('.')
        .map(|(_, v)| v)
        .unwrap_or(qualified_tool);

    let out = match verb {
        "ingest_issue" => {
            let item = ingest_issue(
                &node.store,
                principal,
                ws,
                str_arg(input, "issue_id")?,
                str_arg(input, "payload")?,
                u64_arg(input, "ts")?,
            )
            .await
            .map_err(wf_to_tool)?;
            json!({ "id": item.id, "channel": item.channel })
        }
        "request_approval" => {
            let item = request_approval(
                &node.store,
                principal,
                ws,
                str_arg(input, "approval_id")?,
                str_arg(input, "scope_doc")?,
                str_arg(input, "team")?,
                u64_arg(input, "ts")?,
            )
            .await
            .map_err(wf_to_tool)?;
            json!({ "id": item.id })
        }
        "resolve_approval" => {
            resolve_approval(
                &node.store,
                principal,
                ws,
                str_arg(input, "approval_id")?,
                decision_arg(input)?,
                u64_arg(input, "ts")?,
            )
            .await
            .map_err(wf_to_tool)?;
            json!({ "ok": true })
        }
        "start_job" => {
            let id = start_coding_job(
                node,
                principal,
                ws,
                CodingJob {
                    job_id: str_arg(input, "job_id")?,
                    approval_id: str_arg(input, "approval_id")?,
                    scope_doc: str_arg(input, "scope_doc")?,
                    channel: str_arg(input, "channel")?,
                    pr_key: str_arg(input, "pr_key")?,
                    ts: u64_arg(input, "ts")?,
                },
            )
            .await
            .map_err(wf_to_tool)?;
            json!({ "job_id": id, "started": true })
        }
        _ => return Err(ToolError::NotFound),
    };
    Ok(out)
}

/// Map the workflow gate's outcome onto the MCP tool error. `Denied`/`NotFound` stay opaque; the
/// approval gate's `AwaitingApproval` surfaces as a `BadInput` (the caller asked to start an
/// unapproved job — a distinguishable client error, not a hidden resource).
fn wf_to_tool(e: WorkflowError) -> ToolError {
    match e {
        WorkflowError::Denied => ToolError::Denied,
        WorkflowError::NotFound => ToolError::NotFound,
        WorkflowError::AwaitingApproval => ToolError::BadInput("awaiting approval".into()),
        WorkflowError::Agent(a) => ToolError::Extension(a.to_string()),
        WorkflowError::Bridge(m) => ToolError::Extension(m),
        WorkflowError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn decision_arg(input: &Value) -> Result<Decision, ToolError> {
    match str_arg(input, "decision")? {
        "approved" => Ok(Decision::Approved),
        "rejected" => Ok(Decision::Rejected),
        "deferred" => Ok(Decision::Deferred),
        other => Err(ToolError::BadInput(format!("bad decision: {other}"))),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing string arg: {key}")))
}

fn u64_arg(input: &Value, key: &str) -> Result<u64, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ToolError::BadInput(format!("missing u64 arg: {key}")))
}
