//! The `bus.*` family — direct Zenoh bus introspection/publish over the host bridge (bus scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const BUS: &[HostTool] = &[
    // bus.* — direct Zenoh bus introspection/publish over the host bridge (bus scope).
    HostTool {
        tool: "bus.publish",
        group: "bus",
        description: "publish a message onto a workspace-scoped bus subject",
    },
    HostTool {
        tool: "bus.peers",
        group: "bus",
        description: "the live peers/routers this node is connected to on the mesh",
    },
];
