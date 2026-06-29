//! The viz capability gate — `viz.query` is a host-native MCP tool gated `mcp:viz.query:call`
//! through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first, then capability). The
//! SAME gate every MCP surface uses. A denial is opaque [`VizError::Denied`].
//!
//! NOTE the leash composition (viz transformations scope, "Capabilities"): this gate authorizes the
//! `viz.query` VERB only. Each target the resolver dispatches is RE-authorized against that target
//! tool's OWN cap inside [`crate::call_tool_at_depth`] — there is no render-path bypass. A caller who
//! holds `mcp:viz.query:call` but not `mcp:store.query:call` gets the verb but an empty frame for a
//! store target.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::VizError;

/// Authorize the `viz.query` MCP verb in workspace `ws`. `Ok(())` only if gate 1 (ws) and
/// `mcp:viz.query:call` both pass.
pub fn authorize_viz(principal: &Principal, ws: &str, verb: &str) -> Result<(), VizError> {
    authorize_tool(principal, ws, verb).map_err(|_| VizError::Denied)
}
