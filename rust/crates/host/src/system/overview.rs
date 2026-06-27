//! `system.overview` — the workspace-scoped health snapshot of every subsystem. Gate first
//! (`mcp:system.overview:call`, workspace-first), then gather the cards. Read-only: it mutates
//! nothing, holds nothing — the reply is a pure function of live subsystem state, so a node restart
//! loses nothing (§3.4).

use lb_auth::Principal;

use super::authorize::authorize_system;
use super::collect::collect_services;
use super::model::SystemOverview;
use super::SystemError;
use crate::boot::Node;
use crate::role::Role;

/// The configured node posture as a stable string (config, not a code branch — §3.1). Surfaced so an
/// operator sees which posture they are debugging.
pub(crate) fn role_label(role: Role) -> &'static str {
    match role {
        Role::Edge => "edge",
        Role::Hub => "hub",
        Role::Solo => "solo",
    }
}

/// Read the full system overview for workspace `ws` as `principal`. The workspace is the caller's
/// (the gateway derives it from the token, never the request). Denials are opaque.
pub async fn system_overview(
    node: &Node,
    principal: &Principal,
    ws: &str,
) -> Result<SystemOverview, SystemError> {
    authorize_system(principal, ws, "system.overview")?;
    let services = collect_services(node, ws).await?;
    Ok(SystemOverview {
        ws: ws.to_string(),
        role: role_label(node.role).to_string(),
        services,
    })
}
