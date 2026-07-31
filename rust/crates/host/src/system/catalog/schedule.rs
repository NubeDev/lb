//! The `schedule.*` family — workspace-scoped **global schedules** (global-schedules).
//!
//! A schedule is a first-class record, not a blob inside one flow node's config: one record is
//! referenced by any number of `schedule` flow nodes and dashboard widgets, and editing it moves all
//! of them at once. These verbs are that record's CRUD plus a read-only `evaluate`.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const SCHEDULE: &[HostTool] = &[
    HostTool {
        tool: "schedule.save",
        group: "schedule",
        description: "create or update a global schedule (weekly windows + date exceptions)",
    },
    HostTool {
        tool: "schedule.get",
        group: "schedule",
        description: "read one global schedule",
    },
    HostTool {
        tool: "schedule.list",
        group: "schedule",
        description: "list the workspace's global schedules",
    },
    HostTool {
        tool: "schedule.delete",
        group: "schedule",
        description: "delete a global schedule",
    },
    HostTool {
        tool: "schedule.evaluate",
        group: "schedule",
        description: "evaluate a schedule now: active state, which source won, next transition",
    },
];
