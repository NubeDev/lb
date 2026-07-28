//! The `flows.*` family — the typed-node DAG engine (flows scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const FLOWS: &[HostTool] = &[
    // flows.* — the typed-node DAG engine (flows scope).
    HostTool {
        tool: "flows.save",
        group: "flows",
        description: "create or update a flow definition (nodes + wires)",
    },
    HostTool {
        tool: "flows.get",
        group: "flows",
        description: "read one flow definition",
    },
    HostTool {
        tool: "flows.list",
        group: "flows",
        description: "list the workspace's flows",
    },
    HostTool {
        tool: "flows.delete",
        group: "flows",
        description: "delete a flow",
    },
    HostTool {
        tool: "flows.enable",
        group: "flows",
        description: "enable/disable a flow's triggers",
    },
    HostTool {
        tool: "flows.run",
        group: "flows",
        description: "run a flow now (a manual run)",
    },
    HostTool {
        tool: "flows.inject",
        group: "flows",
        description: "inject a message into a flow node's port",
    },
    HostTool {
        tool: "flows.cancel",
        group: "flows",
        description: "cancel a running flow run",
    },
    HostTool {
        tool: "flows.suspend",
        group: "flows",
        description: "suspend a running flow run",
    },
    HostTool {
        tool: "flows.resume",
        group: "flows",
        description: "resume a suspended flow run",
    },
    HostTool {
        tool: "flows.watch",
        group: "flows",
        description: "watch a flow's live run events",
    },
    HostTool {
        tool: "flows.nodes",
        group: "flows",
        description: "the node-type catalog the flow canvas builds from",
    },
    HostTool {
        tool: "flows.node.get",
        group: "flows",
        description: "read one node of a flow definition",
    },
    HostTool {
        tool: "flows.node.update",
        group: "flows",
        description: "update one node of a flow definition",
    },
    HostTool {
        tool: "flows.node_state",
        group: "flows",
        description: "the per-node live runtime value (the canvas steady-state view)",
    },
    HostTool {
        tool: "flows.patch_run",
        group: "flows",
        description: "patch a suspended run's pending state before resuming",
    },
    HostTool {
        tool: "flows.runs.get",
        group: "flows",
        description: "read one flow run's record",
    },
    HostTool {
        tool: "flows.runs.list",
        group: "flows",
        description: "list a flow's runs",
    },
];
