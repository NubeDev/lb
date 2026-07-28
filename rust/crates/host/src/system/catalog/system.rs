//! The `system.*` topology/status console verbs plus `tools.catalog`, the menu source itself.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const SYSTEM: &[HostTool] = &[
    // system.* — the read-only workspace topology + status console (system-map scope).
    HostTool {
        tool: "system.overview",
        group: "system",
        description: "the per-subsystem health + metrics status grid for the workspace",
    },
    HostTool {
        tool: "system.topology",
        group: "system",
        description: "nodes + wiring edges for the react-flow topology graph",
    },
    HostTool {
        tool: "system.subsystem",
        group: "system",
        description: "the full live detail of one subsystem (the no-page card drill-in)",
    },
    HostTool {
        tool: "system.tools",
        group: "system",
        description: "this catalog — every MCP tool reachable for the workspace, with descriptions",
    },
    HostTool {
        tool: "system.acp",
        group: "system",
        description: "the ACP adapter's static protocol/capability facts",
    },
    // tools.* — the palette/agent menu source itself.
    HostTool {
        tool: "tools.catalog",
        group: "tools",
        description: "the MCP tools you are authorized to call in this workspace",
    },
];
