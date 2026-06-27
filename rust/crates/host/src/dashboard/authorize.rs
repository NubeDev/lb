//! The dashboard capability gate (gates 1+2) — each verb is a host-native MCP tool, gated by
//! `mcp:dashboard.<verb>:call` through the shared `lb_mcp::authorize_tool` chokepoint
//! (workspace-first, then capability — dashboard scope, §3.5/§3.6). The same gate every MCP surface
//! uses; dashboards are not special. Gate 3 (membership/visibility) is a separate check in
//! `visibility.rs`, run strictly *after* this.
//!
//! A denial is opaque [`DashboardError::Denied`] — no existence signal, so an un-granted caller
//! cannot learn what dashboards exist.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::DashboardError;

/// Authorize the `dashboard.<verb>` MCP surface in workspace `ws`. `Ok(())` only if gate 1 (ws) and
/// `mcp:dashboard.<verb>:call` both pass.
pub fn authorize_dashboard(
    principal: &Principal,
    ws: &str,
    verb: &str,
) -> Result<(), DashboardError> {
    authorize_tool(principal, ws, verb).map_err(|_| DashboardError::Denied)
}
