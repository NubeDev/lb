//! The native-tier capability gate — each `native.<verb>` is a host-native MCP tool, gated by
//! `mcp:native.<verb>:call` through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first,
//! then capability). The same gate every MCP surface uses — the native tier is not special, exactly
//! like `authorize_registry`/`authorize_workflow`.
//!
//! This gates the supervisor control plane (install / start / stop / restart / status). Spawning is
//! authority expressed as "may call `native.install`" — NOT a new `process:` capability surface (a
//! surface is a deliberate grammar change; the MCP gate already expresses this). It is independent
//! of the signature gate: being allowed to install never implies an artifact is trustworthy, and a
//! trusted artifact still cannot be installed without the grant. Two gates, both enforced.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::NativeServiceError;

/// Authorize the `native.<verb>` MCP surface in workspace `ws` for `principal`. `Ok(())` only if
/// gate 1 (ws) and `mcp:native.<verb>:call` both pass. Any denial is opaque
/// [`NativeServiceError::Denied`] — no signal about whether a sidecar exists.
pub fn authorize_native(
    principal: &Principal,
    ws: &str,
    verb: &str,
) -> Result<(), NativeServiceError> {
    let tool = format!("native.{verb}");
    authorize_tool(principal, ws, &tool).map_err(|_| NativeServiceError::Denied)
}
