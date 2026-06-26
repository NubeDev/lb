//! The agent invoke gate — the MCP surface check that gates *invoking* the agent at all
//! (`mcp:agent.invoke:call`, workspace-first), via the shared `lb_mcp::authorize_tool` chokepoint.
//!
//! This is gate 1 of the agent flow and runs on the CALLING node, before the loop or any
//! delegation: a caller without `mcp:agent.invoke:call` (or in the wrong workspace) is refused here
//! and the agent never runs. It is independent of the caps the agent then exercises inside the loop
//! — being allowed to invoke the agent never implies the tools/skills/docs it may reach (agent
//! scope). Those are re-checked under the DERIVED principal at each step.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::AgentError;

/// Authorize invoking the agent in workspace `ws` for `principal`. `Ok(())` only if gate 1 (ws) and
/// the `mcp:agent.invoke:call` capability both pass. Any denial is opaque [`AgentError::Denied`].
pub fn authorize_invoke(principal: &Principal, ws: &str) -> Result<(), AgentError> {
    authorize_tool(principal, ws, "agent.invoke").map_err(|_| AgentError::Denied)
}
