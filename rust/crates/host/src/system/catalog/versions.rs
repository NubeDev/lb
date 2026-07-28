//! The `versions.*` verbs — generic entity version history + restore (versions scope, #112).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`. Nothing here names an
//! extension — these are host verbs over the core-owned kind plan table.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const VERSIONS: &[HostTool] = &[
    HostTool {
        tool: "versions.list",
        group: "versions",
        description: "list an entity's saved versions, newest first (metadata only)",
    },
    HostTool {
        tool: "versions.get",
        group: "versions",
        description: "read one saved version's full snapshot",
    },
    HostTool {
        tool: "versions.restore",
        group: "versions",
        description: "restore a saved version by re-saving it as the live record",
    },
    HostTool {
        tool: "versions.config.get",
        group: "versions",
        description: "how many versions this workspace keeps per entity",
    },
    HostTool {
        tool: "versions.config.set",
        group: "versions",
        description: "set how many versions this workspace keeps per entity (admin)",
    },
];
