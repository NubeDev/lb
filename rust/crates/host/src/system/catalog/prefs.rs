//! The `prefs.*` preference axes + the `message.*` recipient-localized rendering verbs.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const PREFS: &[HostTool] = &[
    // prefs.* — member/workspace preference axes (prefs scope).
    HostTool {
        tool: "prefs.get",
        group: "prefs",
        description: "read your raw member preference record",
    },
    HostTool {
        tool: "prefs.set",
        group: "prefs",
        description: "set one of your member preference axes",
    },
    HostTool {
        tool: "prefs.resolve",
        group: "prefs",
        description: "your effective preferences (member over workspace defaults)",
    },
    HostTool {
        tool: "prefs.set_default",
        group: "prefs",
        description: "set a workspace-default preference axis (admin)",
    },
    HostTool {
        tool: "prefs.catalog",
        group: "prefs",
        description: "the preference axes + allowed values",
    },
    // message.* — recipient-localized rendering (i18n-catalogs scope).
    HostTool {
        tool: "message.render",
        group: "message",
        description: "render a message template with values (caller's locale)",
    },
    HostTool {
        tool: "message.render_recipient",
        group: "message",
        description: "render a message template localized to another recipient",
    },
    HostTool {
        tool: "message.set_catalog",
        group: "message",
        description: "write a message-template catalog (admin)",
    },
];
