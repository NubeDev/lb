//! The `inbox.*` / `outbox.*` families — the durable workflow primitives (inbox-outbox scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const WORKFLOW: &[HostTool] = &[
    // inbox.* / outbox.* — the durable workflow primitives (inbox-outbox scope).
    HostTool {
        tool: "inbox.list",
        group: "inbox",
        description: "the durable approvals/triage items awaiting a decision on a channel",
    },
    HostTool {
        tool: "inbox.record",
        group: "inbox",
        description: "create an inbox item (author forced to the caller — not spoofable)",
    },
    HostTool {
        tool: "inbox.resolve",
        group: "inbox",
        description: "settle an inbox item with a decision (idempotent on the item id)",
    },
    HostTool {
        tool: "outbox.status",
        group: "outbox",
        description: "the transactional-effect delivery snapshot (pending/delivered/dead)",
    },
    HostTool {
        tool: "outbox.enqueue",
        group: "outbox",
        description: "stage a must-deliver effect for the outbox relay (with backoff)",
    },
    // The relay half of the outbox contract — dispatched by `tool_call.rs` alongside the two above,
    // but uncataloged until now (so a relay/worker caller could not discover them from the menu).
    HostTool {
        tool: "outbox.enqueue_held",
        group: "outbox",
        description:
            "stage an effect HELD for approval (rides the outbox.enqueue cap; released on approval)",
    },
    HostTool {
        tool: "outbox.due",
        group: "outbox",
        description: "the effects whose backoff has elapsed and are due for delivery now",
    },
    HostTool {
        tool: "outbox.mark_delivered",
        group: "outbox",
        description: "settle one staged effect as delivered (the relay's success ack)",
    },
    HostTool {
        tool: "outbox.mark_failed",
        group: "outbox",
        description: "record a delivery failure on one effect (schedules the next backoff attempt)",
    },
];
