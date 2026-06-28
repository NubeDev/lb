//! The federation MCP gate (datasources scope). Workspace-first, then `mcp:<verb>:call` — the same
//! two-gate chokepoint every host verb runs (rule 5/§3.5/§3.6) before touching a source. A denied
//! caller is opaque, indistinguishable from a missing tool.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::FederationError;

/// Authorize `qualified_tool` (e.g. `federation.query`, `datasource.add`) for `principal` in `ws`.
/// Workspace isolation is checked first, then the per-verb capability.
pub fn authorize(
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
) -> Result<(), FederationError> {
    authorize_tool(principal, ws, qualified_tool).map_err(|_| FederationError::Denied)
}
