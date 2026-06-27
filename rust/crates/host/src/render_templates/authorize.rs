//! The render-template capability gate (gates 1+2) — each verb is a host-native MCP tool gated by
//! `mcp:template.<verb>:call` through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first,
//! then capability). Author-ownership (the gate-3 analog) is checked separately inside `save`/`delete`
//! against the persisted `author`. A denial is opaque [`RenderTemplateError::Denied`].

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::RenderTemplateError;

/// Authorize the `template.<verb>` MCP surface in workspace `ws`. `Ok(())` only if gate 1 (ws) and
/// `mcp:template.<verb>:call` both pass.
pub fn authorize_template(
    principal: &Principal,
    ws: &str,
    verb: &str,
) -> Result<(), RenderTemplateError> {
    authorize_tool(principal, ws, verb).map_err(|_| RenderTemplateError::Denied)
}
