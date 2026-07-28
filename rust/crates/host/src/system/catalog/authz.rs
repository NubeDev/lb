//! The `authz.*` / `grants.*` / `roles.*` / `teams.*` families — the scoped read API + the admin write surface.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const AUTHZ: &[HostTool] = &[
    // authz.* — the scoped read API (entity-scoped-grants scope) + the access-console verbs.
    HostTool {
        tool: "authz.check_scoped",
        group: "authz",
        description:
            "check if the calling principal may reach a record under a cap (entity-scoped)",
    },
    HostTool {
        tool: "authz.scope_filter",
        group: "authz",
        description: "which rows in a table the calling principal may reach under a cap",
    },
    HostTool {
        tool: "authz.delegate_reach",
        group: "authz",
        description:
            "marker cap: hold it to name a `subject` on check_scoped/scope_filter (delegated reach)",
    },
    HostTool {
        tool: "authz.resolve",
        group: "authz",
        description: "resolved effective caps with provenance (access-console; admin-only)",
    },
    HostTool {
        tool: "authz.revoke-tokens",
        group: "authz",
        description: "kill live tokens + tombstone grants for a subject (admin-only)",
    },
    // grants.*/roles.*/teams.* — the authz admin write+read surface (authz-grants scope), reachable
    // over the one MCP bridge (authz-verbs-mcp-dispatch scope) so a native ext can mint scoped grants.
    HostTool {
        tool: "grants.assign",
        group: "authz",
        description: "grant a cap (optionally scoped to rows) to a subject (admin-only)",
    },
    HostTool {
        tool: "grants.revoke",
        group: "authz",
        description: "revoke a granted cap+scope from a subject (admin-only)",
    },
    HostTool {
        tool: "grants.list",
        group: "authz",
        description: "list the caps directly granted to a subject (admin-only)",
    },
    HostTool {
        tool: "grants.list_scoped",
        group: "authz",
        description: "list a subject's grants with their row scopes (admin-only)",
    },
    HostTool {
        tool: "roles.define",
        group: "authz",
        description: "create or replace a role's cap bundle (admin-only)",
    },
    HostTool {
        tool: "roles.list",
        group: "authz",
        description: "list the roles defined in the workspace (admin-only)",
    },
    HostTool {
        tool: "roles.delete",
        group: "authz",
        description: "delete a role and detach its grants (admin-only; built-ins immutable)",
    },
    HostTool {
        tool: "teams.create",
        group: "authz",
        description: "create or rename a team (admin-only)",
    },
    HostTool {
        tool: "teams.list",
        group: "authz",
        description: "list the teams in the workspace (admin-only)",
    },
    HostTool {
        tool: "teams.rename",
        group: "authz",
        description: "rename a team, keeping its grants and members (admin-only)",
    },
    HostTool {
        tool: "teams.delete",
        group: "authz",
        description: "delete a team and cascade its memberships off (admin-only)",
    },
];
