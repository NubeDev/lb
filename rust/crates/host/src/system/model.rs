//! The wire shapes for the system-observability verbs (`system.overview` / `system.topology`).
//! A workspace-scoped, read-only snapshot of every platform subsystem — the data a developer reads
//! to answer "what is connected and is it healthy?" for one workspace. One responsibility: the types;
//! the gathering lives in `collect.rs`, the verbs in `overview.rs`/`topology.rs`.
//!
//! These are derived, not stored (the snapshot is a pure function of live subsystem state + the
//! embedded store at call time), so a node restart loses nothing — re-deriving is the whole design.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A coarse health rollup for one subsystem. `Idle` is *up but nothing flowing* (an empty queue is
/// healthy, not broken) — kept distinct from `Ok` so the UI can grey it rather than green it, and
/// from `Degraded` so an empty inbox never reads as a problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    /// Up and nominal.
    Ok,
    /// Up, but nothing is flowing (e.g. an empty queue). Informational, not a fault.
    Idle,
    /// Up but something wants attention (dead-lettered effects, an enabled-but-stopped extension).
    Degraded,
    /// Not reachable.
    Down,
}

/// One labelled number on a subsystem card (e.g. `{label: "dead-letter", value: "2"}`). String-typed
/// so heterogeneous metrics (counts, a role name, "native ×3") share one shape the UI renders flat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metric {
    pub label: String,
    pub value: String,
}

impl Metric {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

/// The status of one platform subsystem for a workspace — one card in the status grid and one node
/// in the topology graph. `id` is the stable key the topology edges reference; `group` buckets the
/// card (motion / state / runtime / workflow), mirroring the core's state-vs-motion split (§3.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatus {
    /// Stable key: `gateway`, `bus`, `mcp`, `store`, `ingest`, `inbox`, `outbox`, `jobs`,
    /// `extensions`, `registry`. Topology edges reference these.
    pub id: String,
    pub label: String,
    /// `motion` (bus/gateway), `state` (store/ingest), `workflow` (inbox/outbox/jobs),
    /// `runtime` (mcp/extensions/registry).
    pub group: String,
    pub health: Health,
    /// A one-line human summary for the card subtitle.
    pub detail: String,
    pub metrics: Vec<Metric>,
}

/// `system.overview` — the workspace-scoped health snapshot of every subsystem. `role` is the node's
/// configured posture (edge/hub/solo); it is config, not a code branch (§3.1), surfaced here only so
/// the operator sees which posture they are debugging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemOverview {
    pub ws: String,
    pub role: String,
    pub services: Vec<ServiceStatus>,
}

/// `system.subsystem` — the full status of ONE subsystem plus a subsystem-specific `extra` blob.
/// The detail view a no-page card (gateway/bus/mcp) opens: the same [`ServiceStatus`] the grid shows,
/// plus opaque extra detail the grid has no room for. For `bus` the extra carries the live peer/router
/// zid lists (`{ "peer_zids": [...], "router_zids": [...] }`); for every other subsystem it is an empty
/// object. Read-only and derived, like the rest of the map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubsystemDetail {
    pub ws: String,
    pub role: String,
    /// The full card for this subsystem (same shape the overview grid renders).
    pub service: ServiceStatus,
    /// Subsystem-specific detail the card has no room for. `{}` unless the subsystem has extra to
    /// show (today: `bus` → its connected peer/router zids).
    pub extra: Value,
}

/// A node in the topology graph — a 1:1 projection of a [`ServiceStatus`] minus the metrics (the
/// graph shows shape + health; the cards show the numbers).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopoNode {
    pub id: String,
    pub label: String,
    pub group: String,
    pub health: Health,
}

/// A directed edge in the topology graph: `from` reaches `to`, annotated with how (`label`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopoEdge {
    pub from: String,
    pub to: String,
    pub label: String,
}

/// `system.topology` — nodes + edges for the react-flow wiring view. The nodes carry live health;
/// the edges are the architectural wiring (which subsystem reaches which), filtered to the nodes
/// actually present so the graph never dangles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemTopology {
    pub ws: String,
    pub role: String,
    pub nodes: Vec<TopoNode>,
    pub edges: Vec<TopoEdge>,
}
