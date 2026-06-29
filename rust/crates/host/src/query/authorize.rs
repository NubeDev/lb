//! The query MCP gate (query scope). Workspace-first, then `mcp:<verb>:call` — the same two-gate
//! chokepoint every host verb runs (rule 5) before touching a saved query. A denied caller is opaque,
//! indistinguishable from a missing tool.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::QueryError;

/// Authorize `qualified_tool` (e.g. `query.run`, `query.save`) for `principal` in `ws`. Workspace
/// isolation is checked first, then the per-verb capability.
pub fn authorize(principal: &Principal, ws: &str, qualified_tool: &str) -> Result<(), QueryError> {
    authorize_tool(principal, ws, qualified_tool).map_err(|_| QueryError::Denied)
}
