//! `system.acp` — the ACP (Agent Client Protocol) adapter's static capability/protocol facts. The read
//! behind the ACP service page. Gate first (`mcp:system.acp:call`, workspace-first, admin-only by the
//! same grant convention as the other `system.*` verbs), then return the host-owned facts.
//!
//! ACP is a per-stdio-session adapter (agent-run Part 4), not a polled network server, so this is
//! *reachable capability info*, not a live health feed — there is nothing workspace-specific and
//! nothing to mutate. The facts come from [`super::acp::acp_info`] (which mirrors the acp role's
//! handshake); the gate is still workspace-first so the page is admin-only like the rest of the map.

use lb_auth::Principal;

use super::acp::acp_info;
use super::authorize::authorize_system;
use super::model::AcpInfo;
use super::SystemError;

/// Return the ACP adapter's static facts for workspace `ws` as `principal`. The facts are node-level
/// (not workspace-specific), but the gate is workspace-first so the verb is admin-only like its
/// siblings. Denials are opaque.
pub async fn system_acp(principal: &Principal, ws: &str) -> Result<AcpInfo, SystemError> {
    authorize_system(principal, ws, "system.acp")?;
    Ok(acp_info())
}
