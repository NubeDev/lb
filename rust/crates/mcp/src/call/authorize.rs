//! The authorize phase — the only gate to dispatch. Builds the `mcp:<ext>.<tool>:call`
//! request and runs the shared `caps::check` chokepoint (workspace-first, then capability).
//! Any denial collapses to [`ToolError::Denied`] with no existence signal (mcp scope).

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};

use super::error::ToolError;

/// Authorize a call to `qualified_tool` (`<ext>.<tool>`) in workspace `ws`. The capability
/// resource is the qualified tool name verbatim, so `mcp:hello.echo:call` /
/// `mcp:hello.*:call` match exactly per the grammar.
pub fn authorize(principal: &Principal, ws: &str, qualified_tool: &str) -> Result<(), ToolError> {
    let req = Request::new(ws, Surface::Mcp, qualified_tool, Action::Call);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        // Both gates (workspace, capability) collapse to one opaque Denied.
        Decision::Denied(_) => Err(ToolError::Denied),
    }
}
