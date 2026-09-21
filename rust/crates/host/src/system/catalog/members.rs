//! The `members.*` family — team membership: who is on a team, and who may change that.
//!
//! Distinct from `teams.*` (the team RECORD, under `authz`) and from `membership.*` (WORKSPACE
//! membership, under `identity`): this family writes and reads the `member` EDGE between a team and
//! a user. It is the edge `case.assignees` walks to answer "who do I share a team with?", so with no
//! reachable verb here every assign picker in the product was empty.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const MEMBERS: &[HostTool] = &[
    HostTool {
        tool: "members.add",
        group: "members",
        description: "add a user to a team (team-admin)",
    },
    HostTool {
        tool: "members.list",
        group: "members",
        description: "list the users on a team",
    },
    HostTool {
        tool: "members.remove",
        group: "members",
        description: "remove a user from a team (admin-only)",
    },
];
