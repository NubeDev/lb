//! The `policy.sla.*` family — the SLA service policies a case's deadlines are computed from
//! (case-plane scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`. Listed here for the
//! same reason every other family is: the console and the agent's `tools.catalog`-derived menu are
//! built from this inventory, so a verb absent from it is dispatchable but **invisible** — nobody
//! can find it to call it.
//!
//! Both verbs are admin. The catalog is gated by `gate_tool_for`, so a member simply does not see
//! these rows — the cardinal rule holds ("advertise a tool only if the call would allow it").

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const POLICY: &[HostTool] = &[
    HostTool {
        tool: "policy.sla.set",
        group: "policy",
        description: "upsert an SLA service policy (match site/category/severity, respond/resolve \
                      hours, business calendar); admin",
    },
    HostTool {
        tool: "policy.sla.list",
        group: "policy",
        description: "list SLA service policies, most-specific match first — the order the \
                      resolver applies them in; admin",
    },
];
