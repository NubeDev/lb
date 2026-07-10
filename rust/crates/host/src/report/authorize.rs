//! The report capability gate (gates 1+2) — each verb is a host-native MCP tool, gated by
//! `mcp:report.<verb>:call` through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first,
//! then capability). Mirrors `authorize_panel`. Gate 3 (membership/visibility) is a separate check
//! in `visibility.rs`, run strictly *after* this. A denial is opaque [`ReportError::Denied`].

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::ReportError;

/// Authorize the `report.<verb>` MCP surface in workspace `ws`. `Ok(())` only if gate 1 (ws) and
/// `mcp:report.<verb>:call` both pass.
pub fn authorize_report(principal: &Principal, ws: &str, verb: &str) -> Result<(), ReportError> {
    authorize_tool(principal, ws, verb).map_err(|_| ReportError::Denied)
}
