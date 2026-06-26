//! The registry capability gate — each `registry.<verb>` is a host-native MCP tool, gated by
//! `mcp:registry.<verb>:call` through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first,
//! then capability). The same gate every MCP surface uses; the registry is not special — exactly like
//! `authorize_workflow`.
//!
//! This gates the registry-client verbs (pull / list / install). It is independent of the signature
//! gate (`verify_artifact`): being allowed to pull never implies an artifact is trustworthy, and a
//! trusted artifact still cannot be pulled without the grant. Two gates, both enforced.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::RegistryServiceError;

/// Authorize the `registry.<verb>` MCP surface in workspace `ws` for `principal`. `Ok(())` only if
/// gate 1 (ws) and `mcp:registry.<verb>:call` both pass. Any denial is opaque
/// [`RegistryServiceError::Denied`] — no signal about whether the artifact exists.
pub fn authorize_registry(
    principal: &Principal,
    ws: &str,
    verb: &str,
) -> Result<(), RegistryServiceError> {
    let tool = format!("registry.{verb}");
    authorize_tool(principal, ws, &tool).map_err(|_| RegistryServiceError::Denied)
}
