//! `system.tools` — the full catalog of MCP tools reachable for one workspace: the built-in
//! host-native verbs plus every extension-contributed tool in the runtime registry. The read behind
//! the MCP service page's tool table. Gate first (`mcp:system.tools:call`, workspace-first, admin-only
//! by grant convention — the same single gate the other `system.*` verbs run), then build the catalog.
//!
//! Read-only and derived live (registry + a static host catalog), like the rest of the map — it
//! mutates nothing and owns no record, so a node restart loses nothing.

use lb_auth::Principal;

use super::authorize::authorize_system;
use super::collect::collect_tools;
use super::model::SystemTools;
use super::overview::role_label;
use super::SystemError;
use crate::boot::Node;

/// Read the full tool catalog for workspace `ws` as `principal`. The workspace is the caller's (the
/// gateway derives it from the token, never the request). Denials are opaque.
pub async fn system_tools(
    node: &Node,
    principal: &Principal,
    ws: &str,
) -> Result<SystemTools, SystemError> {
    authorize_system(principal, ws, "system.tools")?;
    Ok(SystemTools {
        ws: ws.to_string(),
        role: role_label(node.role).to_string(),
        tools: collect_tools(node),
    })
}
