//! The `party.*` family — the external parties a case can ask something of (case-plane scope,
//! wave 2).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`. Listed here for the
//! reason every family is: the console and the agent's `tools.catalog`-derived menu are built from
//! this inventory, and a dispatched verb absent from it is reachable but invisible.
//!
//! Both verbs are admin, so the `gate_tool_for`-gated catalog simply does not show these rows to a
//! member. **Rule 10**: nothing here names a party — these are the two verbs that read and write
//! whatever the workspace puts in its own roster.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const PARTY: &[HostTool] = &[
    HostTool {
        tool: "party.upsert",
        group: "party",
        description: "create or replace one party in the workspace roster (contractor / fm / \
                      client / fms, contact, sites, trades, ask window); admin",
    },
    HostTool {
        tool: "party.list",
        group: "party",
        description: "the party roster, by name, optionally narrowed to a kind or a site; admin",
    },
];
