//! The `secret.*` family — the extension-owned, host-mediated secret CRUD surface (secrets scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const SECRET: &[HostTool] = &[
    // secret.* — the extension-owned, host-mediated secret CRUD surface (secrets scope). list
    // returns metadata only; only get (three-gate) ever returns a value.
    HostTool {
        tool: "secret.set",
        group: "secret",
        description: "store (create/overwrite) a secret, owner-stamped and private by default",
    },
    HostTool {
        tool: "secret.get",
        group: "secret",
        description: "read a secret value (owner for private, any member for workspace-shared)",
    },
    HostTool {
        tool: "secret.set_visibility",
        group: "secret",
        description: "owner-only toggle of a secret's visibility (private | workspace)",
    },
    HostTool {
        tool: "secret.delete",
        group: "secret",
        description: "owner-only delete of a secret",
    },
    HostTool {
        tool: "secret.list",
        group: "secret",
        description: "list secret metadata (path/owner/visibility) — never the values",
    },
];
