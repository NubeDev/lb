//! The `mail.*` family — watched mailboxes (mail-source scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const MAIL: &[HostTool] = &[
    HostTool {
        tool: "mail.source.register",
        group: "mail",
        description: "watch an IMAP mailbox (arriving mail becomes assets, series and inbox items)",
    },
    HostTool {
        tool: "mail.source.update",
        group: "mail",
        description: "amend a mail source's configuration (its cursor and history are kept)",
    },
    HostTool {
        tool: "mail.source.list",
        group: "mail",
        description: "list the workspace's mail sources",
    },
    HostTool {
        tool: "mail.source.get",
        group: "mail",
        description: "read one mail source",
    },
    HostTool {
        tool: "mail.source.delete",
        group: "mail",
        description: "delete a mail source (the subscription only — imports are kept)",
    },
    HostTool {
        tool: "mail.source.pause",
        group: "mail",
        description: "pause or resume polling a mail source",
    },
    HostTool {
        tool: "mail.source.check",
        group: "mail",
        description:
            "test a mailbox's credentials and peek at its newest message (imports nothing)",
    },
    HostTool {
        tool: "mail.source.poll",
        group: "mail",
        description: "poll a mailbox now (this one imports and advances the cursor)",
    },
    HostTool {
        tool: "mail.formats",
        group: "mail",
        description: "the file formats this node can decode into series samples",
    },
];
