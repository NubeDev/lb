//! The system-map service — the host's capability chokepoint for the admin, **read-only** workspace
//! topology + status console (system-map scope). Beside `dbview`/`dashboard`, it reads the booted
//! `Node`'s subsystem handles directly (an extension can't observe the runtime that supervises it),
//! rolling them into one snapshot with two projections: a status grid (`system.overview`) and a
//! react-flow wiring graph (`system.topology`).
//!
//! **The headline decision:** both verbs authorize **once** (`mcp:system.overview|topology:call`,
//! workspace-first §7, admin-only by grant convention) and then read **raw** subsystem state through
//! the shared `collect.rs` — NOT through the gated host wrappers (`ext_list`, `outbox_status`), which
//! re-check their own caps. The snapshot is one capability, not the union of every verb it summarizes
//! (mirroring how `dbview` runs its admin gate once, then calls the raw `lb_store::tables`). Read-only
//! by design — control verbs (`ext.enable`/`disable`, lifecycle) stay in their own scopes.
//!
//! The verbs (one per file, FILE-LAYOUT):
//!   - `system.overview` ([`system_overview`]) — the per-subsystem status grid.
//!   - `system.topology` ([`system_topology`]) — nodes + fixed wiring edges for react-flow.
//!   - `system.subsystem` ([`system_subsystem`]) — the detail of ONE subsystem (+ a `bus` zid blob),
//!     so a no-page card drills into a real view instead of dead-ending.
//!   - `system.tools` ([`system_tools`]) — the full catalog of reachable MCP tools (host-native +
//!     extension-contributed), with descriptions, for the MCP service page (tool-catalog scope).
//!   - `system.acp` ([`system_acp`]) — the ACP adapter's static protocol/capability facts, for the
//!     ACP service page (tool-catalog scope).
//!   - the MCP bridge ([`call_system_tool`]) — the one MCP contract over all five.

mod acp;
mod acp_verb;
mod authorize;
mod catalog;
mod collect;
mod error;
mod model;
mod overview;
mod subsystem;
mod tool;
mod tools;
mod topology;

pub use acp_verb::system_acp;
pub use authorize::authorize_system;
pub(crate) use catalog::host_catalog;
pub use error::SystemError;
pub use model::{
    AcpInfo, Health, Metric, ServiceStatus, SubsystemDetail, SystemOverview, SystemTools,
    SystemTopology, ToolInfo, TopoEdge, TopoNode,
};
pub use overview::system_overview;
pub use subsystem::system_subsystem;
pub use tool::call_system_tool;
pub use tools::system_tools;
pub use topology::system_topology;
