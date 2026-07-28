//! The `reminder.*` family — scheduled reminders (reminders-tenant scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const REMINDER: &[HostTool] = &[
    // reminder.* — scheduled reminders (reminders-tenant scope).
    HostTool {
        tool: "reminder.create",
        group: "reminder",
        description: "create a scheduled reminder",
    },
    HostTool {
        tool: "reminder.update",
        group: "reminder",
        description: "update a reminder",
    },
    HostTool {
        tool: "reminder.delete",
        group: "reminder",
        description: "delete a reminder",
    },
    HostTool {
        tool: "reminder.list",
        group: "reminder",
        description: "list the workspace's reminders",
    },
    HostTool {
        tool: "reminder.fire",
        group: "reminder",
        description: "fire a reminder now (the gated run-now control)",
    },
];
