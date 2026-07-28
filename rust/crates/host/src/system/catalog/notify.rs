//! The `device.*` / `notify.*` families — the push-notification surface (push-target scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const NOTIFY: &[HostTool] = &[
    // device.* / notify.* — the push-notification surface (push-target scope).
    HostTool {
        tool: "device.register",
        group: "notify",
        description: "register a push device (self-only, upsert by token)",
    },
    HostTool {
        tool: "device.list",
        group: "notify",
        description: "list the caller's own registered devices",
    },
    HostTool {
        tool: "device.remove",
        group: "notify",
        description: "remove a registered device (self-only)",
    },
    HostTool {
        tool: "notify.send",
        group: "notify",
        description: "enqueue a push notification to an audience (outbox-delivered)",
    },
];
