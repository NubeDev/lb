//! The `rules.*` family — rule authoring + evaluation (rules-workbench scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const RULES: &[HostTool] = &[
    // rules.* — rule authoring + evaluation (rules-workbench scope).
    HostTool {
        tool: "rules.save",
        group: "rules",
        description: "create or update a rule",
    },
    HostTool {
        tool: "rules.get",
        group: "rules",
        description: "read one rule",
    },
    HostTool {
        tool: "rules.list",
        group: "rules",
        description: "list the workspace's rules",
    },
    HostTool {
        tool: "rules.delete",
        group: "rules",
        description: "delete a rule",
    },
    HostTool {
        tool: "rules.run",
        group: "rules",
        description: "run a rule now against real data",
    },
    HostTool {
        tool: "rules.eval",
        group: "rules",
        description: "evaluate a rule expression without saving it",
    },
    HostTool {
        tool: "rules.help",
        group: "rules",
        description: "the rule grammar + function reference",
    },
    HostTool {
        tool: "rules.run_async",
        group: "rules",
        description: "run a rule as a durable background job (long-running; pausable/resumable)",
    },
    HostTool {
        tool: "rules.runs.get",
        group: "rules",
        description: "one background rule run: status, progress, checkpoints, result",
    },
    HostTool {
        tool: "rules.runs.list",
        group: "rules",
        description: "list the workspace's background rule runs",
    },
    HostTool {
        tool: "rules.runs.suspend",
        group: "rules",
        description: "pause a background rule run (parks at the next operation; resumable)",
    },
    HostTool {
        tool: "rules.runs.resume",
        group: "rules",
        description: "resume a paused/orphaned rule run (replays over its checkpoints)",
    },
    HostTool {
        tool: "rules.runs.cancel",
        group: "rules",
        description: "cancel a background rule run (terminal; transcript kept)",
    },
];
