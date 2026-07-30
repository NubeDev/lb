//! The `insight.*` family — the durable data-insight record, occurrences, subscriptions and policy.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const INSIGHT: &[HostTool] = &[
    // insight.* — the durable data-insight record + occurrences + subscriptions + policy
    // (insights umbrella scope + occurrences/subscriptions/notify sub-scopes).
    HostTool {
        tool: "insight.raise",
        group: "insight",
        description: "raise a data-insight (dedup on (ws, dedup_key); bumps count or re-opens)",
    },
    HostTool {
        tool: "insight.get",
        group: "insight",
        description: "read one insight by id",
    },
    HostTool {
        tool: "insight.list",
        group: "insight",
        description: "faceted, keyset-paged list of insights (status/severity/origin/tags/range)",
    },
    HostTool {
        tool: "insight.watch",
        group: "insight",
        description: "SSE live feed of raise/ack/resolve events on ws/{ws}/insight/events",
    },
    HostTool {
        tool: "insight.ack",
        group: "insight",
        description: "ack an insight (open → acked; acked_by host-forced)",
    },
    HostTool {
        tool: "insight.resolve",
        group: "insight",
        description: "resolve an insight (* → resolved; idempotent)",
    },
    HostTool {
        tool: "insight.assign",
        group: "insight",
        description: "assign/re-assign/un-assign an insight's owner (subject: user: or team:; bulk via ids, max 100)",
    },
    HostTool {
        tool: "insight.comment",
        group: "insight",
        description: "append a comment to an insight's thread (author host-stamped; append-only, never evicted)",
    },
    HostTool {
        tool: "insight.delete",
        group: "insight",
        description: "hard-delete an insight and cascade its occurrence ring (idempotent)",
    },
    HostTool {
        tool: "insight.occurrences",
        group: "insight",
        description: "read the per-insight occurrence ring (newest-first, keyset-paged)",
    },
    HostTool {
        tool: "insight.occurrence.delete",
        group: "insight",
        description: "delete one row from an insight's occurrence ring by oseq (idempotent)",
    },
    HostTool {
        tool: "insight.sub.create",
        group: "insight",
        description: "subscribe a channel to insights matching a filter (owner host-stamped)",
    },
    HostTool {
        tool: "insight.sub.list",
        group: "insight",
        description: "list subscriptions (own; admin lens: all ws subs)",
    },
    HostTool {
        tool: "insight.sub.get",
        group: "insight",
        description: "read one subscription by id",
    },
    HostTool {
        tool: "insight.sub.delete",
        group: "insight",
        description: "delete a subscription (owner-or-admin; idempotent)",
    },
    HostTool {
        tool: "insight.sub.mute",
        group: "insight",
        description: "toggle a subscription's muted flag (owner only)",
    },
    HostTool {
        tool: "insight.policy.get",
        group: "insight",
        description: "read the workspace notify policy (compiled defaults if absent)",
    },
    HostTool {
        tool: "insight.policy.set",
        group: "insight",
        description: "write the workspace notify policy (admin-only; ring-cap bounded)",
    },
];
