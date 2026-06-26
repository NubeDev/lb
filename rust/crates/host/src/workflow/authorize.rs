//! The workflow capability gate — each `workflow.<verb>` is a host-native MCP tool, gated by
//! `mcp:workflow.<verb>:call` through the shared `lb_mcp::authorize_tool` chokepoint (workspace-
//! first, then capability). Same gate every MCP surface uses; the workflow is not special.
//!
//! These gate the orchestration verbs (ingest / triage / request_approval / resolve_approval /
//! start_job). They are independent of the caps the agent then exercises *inside* the coding job —
//! being allowed to start a workflow never implies the tools the agent may call (those re-run the
//! S5 intersection under the derived principal). Two surfaces, both enforced (coding-workflow scope).

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::WorkflowError;

/// Authorize the `workflow.<verb>` MCP surface in workspace `ws` for `principal`. `Ok(())` only if
/// gate 1 (ws) and `mcp:workflow.<verb>:call` both pass. Any denial is opaque [`WorkflowError::Denied`].
pub fn authorize_workflow(
    principal: &Principal,
    ws: &str,
    verb: &str,
) -> Result<(), WorkflowError> {
    let tool = format!("workflow.{verb}");
    authorize_tool(principal, ws, &tool).map_err(|_| WorkflowError::Denied)
}
