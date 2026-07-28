//! The `channel.*` family — the host's messaging plane (rules-messaging scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const CHANNEL: &[HostTool] = &[
    // channel.* — the host's messaging plane (rules-messaging scope).
    HostTool {
        tool: "channel.create",
        group: "channel",
        description:
            "register a channel so it is listable before the first post (bus pub cap re-checked)",
    },
    HostTool {
        tool: "channel.post",
        group: "channel",
        description: "post a message to a channel (bus cap re-checked per channel)",
    },
    HostTool {
        tool: "channel.list",
        group: "channel",
        description: "list the workspace's channels",
    },
    HostTool {
        tool: "channel.history",
        group: "channel",
        description: "read a channel's persisted message history",
    },
    HostTool {
        tool: "channel.edit",
        group: "channel",
        description: "edit one of your own channel messages",
    },
    HostTool {
        tool: "channel.delete",
        group: "channel",
        description: "delete one of your own channel messages",
    },
    HostTool {
        tool: "channel.chart_pref.get",
        group: "channel",
        description: "read your per-viewer chart preference for a query result",
    },
    HostTool {
        tool: "channel.chart_pref.set",
        group: "channel",
        description: "set your per-viewer chart preference for a query result",
    },
];
