//! The `panel.*` family — the reusable + standalone library-panel asset (library-panels scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const PANEL: &[HostTool] = &[
    // panel.* — the reusable + standalone library-panel asset (library-panels scope).
    HostTool {
        tool: "panel.get",
        group: "panel",
        description: "read one library panel by id (full spec)",
    },
    HostTool {
        tool: "panel.list",
        group: "panel",
        description: "list the library panels visible to the caller",
    },
    HostTool {
        tool: "panel.save",
        group: "panel",
        description: "create or update a library panel the caller owns",
    },
    HostTool {
        tool: "panel.delete",
        group: "panel",
        description: "delete a library panel the caller owns (refused while in use unless forced)",
    },
    HostTool {
        tool: "panel.share",
        group: "panel",
        description: "share a library panel with a team / set its visibility",
    },
    HostTool {
        tool: "panel.usage",
        group: "panel",
        description: "list the dashboards that reference a library panel",
    },
];
