//! `system.topology` — the react-flow wiring view. Gate first (`mcp:system.topology:call`,
//! workspace-first), gather the **same** `ServiceStatus[]` the overview projects from (so the grid
//! and the graph can never disagree, §goals), then project it into graph nodes + the platform's fixed
//! architectural edges. Read-only: it mutates nothing, holds nothing.
//!
//! The node *health* is live (from the gathered cards); the edges are the platform's fixed shape
//! (gateway → mcp, mcp → store/bus, ingest → store, jobs → outbox, …). Every edge is filtered to the
//! nodes actually present so the graph never dangles (a card we did not emit drops its edges too).

use lb_auth::Principal;

use super::authorize::authorize_system;
use super::collect::collect_services;
use super::model::{SystemTopology, TopoEdge, TopoNode};
use super::overview::role_label;
use super::SystemError;
use crate::boot::Node;

/// The architectural wiring of the platform: `(from, to, how)`. This is the **fixed shape** of the
/// core (who reaches whom), not a live read — health rides the nodes, not the edges. Any endpoint not
/// present as a node is filtered out at projection time so the graph never dangles.
const WIRING: &[(&str, &str, &str)] = &[
    ("gateway", "mcp", "MCP dispatch"),
    ("mcp", "store", "reads/writes state"),
    ("mcp", "bus", "publishes motion"),
    ("mcp", "extensions", "host-mediated calls"),
    ("ingest", "store", "drains into series"),
    ("inbox", "store", "durable approvals"),
    ("jobs", "outbox", "must-deliver effects"),
    ("jobs", "store", "resumable sessions"),
    ("registry", "extensions", "installs artifacts"),
    ("extensions", "mcp", "expose tools"),
    ("outbox", "bus", "delivers effects"),
];

/// Read the full system topology for workspace `ws` as `principal`. Same gather as `system_overview`,
/// projected into nodes (1:1, minus the metrics) + the fixed wiring edges, filtered to present nodes.
pub async fn system_topology(
    node: &Node,
    principal: &Principal,
    ws: &str,
) -> Result<SystemTopology, SystemError> {
    authorize_system(principal, ws, "system.topology")?;
    let services = collect_services(node, ws).await?;

    let nodes: Vec<TopoNode> = services
        .iter()
        .map(|s| TopoNode {
            id: s.id.clone(),
            label: s.label.clone(),
            group: s.group.clone(),
            health: s.health,
        })
        .collect();

    let present: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let edges: Vec<TopoEdge> = WIRING
        .iter()
        .filter(|(from, to, _)| present.contains(from) && present.contains(to))
        .map(|(from, to, label)| TopoEdge {
            from: (*from).into(),
            to: (*to).into(),
            label: (*label).into(),
        })
        .collect();

    Ok(SystemTopology {
        ws: ws.to_string(),
        role: role_label(node.role).to_string(),
        nodes,
        edges,
    })
}
