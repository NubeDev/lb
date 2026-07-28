//! The `invite.*` token onboarding surface + the `identity.*` directory/credential admin verbs.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const IDENTITY: &[HostTool] = &[
    // invite.* — the token onboarding surface (invites scope). Accept is pre-auth (gateway route).
    HostTool {
        tool: "invite.create",
        group: "invite",
        description: "mint a single-use invite token (enqueues email delivery; admin-only)",
    },
    HostTool {
        tool: "invite.list",
        group: "invite",
        description: "list invites in the workspace with status (admin-only)",
    },
    HostTool {
        tool: "invite.revoke",
        group: "invite",
        description: "revoke a pending invite (admin-only)",
    },
    HostTool {
        tool: "invite.resend",
        group: "invite",
        description: "resend a pending invite with a fresh token (admin-only)",
    },
    // identity.* — the credential-management admin verb (login-hardening scope). The directory
    // verbs (create/get/list/workspaces) also have dedicated admin REST routes.
    HostTool {
        tool: "identity.set_credential",
        group: "identity",
        description:
            "admin: set/rotate a user's login password (argon2-hashed; never returns the hash)",
    },
    HostTool {
        tool: "identity.create",
        group: "identity",
        description: "admin: create a directory identity (sub, display name, email)",
    },
    HostTool {
        tool: "identity.get",
        group: "identity",
        description: "admin: read one directory identity by sub",
    },
    HostTool {
        tool: "identity.list",
        group: "identity",
        description: "admin: list the directory identities",
    },
    HostTool {
        tool: "identity.workspaces",
        group: "identity",
        description: "admin: the workspaces one identity is a member of",
    },
    HostTool {
        tool: "identity.set_email",
        group: "identity",
        description: "admin: set a user's email address (the email-login handle)",
    },
    HostTool {
        tool: "identity.set_password",
        group: "identity",
        description:
            "admin: set/rotate a user's GLOBAL password (argon2-hashed; never returns the hash)",
    },
];
