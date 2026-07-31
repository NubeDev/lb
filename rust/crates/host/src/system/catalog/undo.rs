//! The `undo` / `redo` / `history.*` verbs — the compensation log (undo scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const UNDO: &[HostTool] = &[
    // history/undo — the compensation log (undo scope).
    HostTool {
        tool: "history.list",
        group: "history",
        description: "list the workspace's mutation history; the result also carries \
                      can_undo/can_redo gate flags for a caller that only enables buttons",
    },
    HostTool {
        tool: "history.compensations",
        group: "history",
        description: "the compensations available for a history entry",
    },
    HostTool {
        tool: "undo",
        group: "history",
        description: "undo your latest undoable mutation",
    },
    HostTool {
        tool: "redo",
        group: "history",
        description: "redo your latest undone mutation",
    },
];
