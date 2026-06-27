//! `system.subsystem` — the detail view for ONE subsystem. The third read verb (beside
//! `overview`/`topology`), so a status card with no owning page (gateway/bus/mcp) can drill into a
//! real detail surface instead of being a dead end. Gate first (`mcp:system.subsystem:call`,
//! workspace-first, admin-only by grant convention — the same single gate the other two run), then
//! gather the **same** `collect_services` snapshot and pick out the requested card, plus a
//! subsystem-specific `extra` blob (for `bus`, its live peer/router zid lists; `{}` otherwise).
//!
//! Read-only and derived, like the rest of the map. An unknown id is handled opaquely — `Denied`,
//! the same answer a no-cap caller gets, so the verb leaks no "which ids exist" signal and never
//! panics.

use lb_auth::Principal;

use super::authorize::authorize_system;
use super::collect::{collect_extra, collect_services};
use super::model::SubsystemDetail;
use super::overview::role_label;
use super::SystemError;
use crate::boot::Node;

/// Read the full detail of subsystem `id` for workspace `ws` as `principal`. The workspace is the
/// caller's (the gateway derives it from the token, never the request). A denial — including an
/// unknown id — is opaque.
pub async fn system_subsystem(
    node: &Node,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<SubsystemDetail, SystemError> {
    authorize_system(principal, ws, "system.subsystem")?;
    let services = collect_services(node, ws).await?;
    let service = services
        .into_iter()
        .find(|s| s.id == id)
        .ok_or(SystemError::Denied)?;
    let extra = collect_extra(node, id).await;
    Ok(SubsystemDetail {
        ws: ws.to_string(),
        role: role_label(node.role).to_string(),
        service,
        extra,
    })
}
