//! The `case.*` family — the case plane: a piece of work that cites insights.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const CASE: &[HostTool] = &[
    // case.* — the durable case record + its members + its history + the triage write path
    // (case-plane scope). An insight is a detection; a case is the work.
    HostTool {
        tool: "case.get",
        group: "case",
        description: "read one case by id",
    },
    HostTool {
        tool: "case.list",
        group: "case",
        description: "the work lanes (mine/waiting/watching), due_at asc then severity desc, with a member count",
    },
    HostTool {
        tool: "case.members",
        group: "case",
        description: "the insights a case cites, paged",
    },
    HostTool {
        tool: "case.events",
        group: "case",
        description: "a case's append-only history, newest-first and paged",
    },
    HostTool {
        tool: "case.open",
        group: "case",
        description: "open a case over one or more insights",
    },
    HostTool {
        tool: "case.merge",
        group: "case",
        description: "fold one case into another; the losing case closes as duplicate",
    },
    HostTool {
        tool: "case.split",
        group: "case",
        description: "pull members out of a case into a new one",
    },
    HostTool {
        tool: "case.workflow",
        group: "case",
        description: "transition to_action/actioned/waiting_on_po/resolved (resolved requires a resolution)",
    },
    HostTool {
        tool: "case.assign",
        group: "case",
        description: "set, re-assign or clear a case's owner (a user: or team: subject)",
    },
    HostTool {
        tool: "case.snooze",
        group: "case",
        description: "park a case until a stated time for a stated reason (until <= now un-snoozes)",
    },
    // case-plane scope §7: the detector feedback loop. SINGULAR `rule.` — a different family from
    // the `rules.` engine beside it, and read-only.
    HostTool {
        tool: "rule.scorecard",
        group: "case",
        description: "detector precision per origin.ref per site over resolved cases: fixed / (fixed + false_positive + self_cleared), plus the median time to fix",
    },
    HostTool {
        tool: "case.comment",
        group: "case",
        description: "append a note to a case's history",
    },
    // The external-party round trip (wave 2). Every one of these is dispatched, so every one needs
    // a row: `host_catalog_covers_dispatch_prefixes` asserts the inventory covers the dispatcher's
    // own list, and a dispatched verb missing from here is reachable but INVISIBLE — nobody can
    // find it to call it. The catalog is gated by `gate_tool_for`, so a viewer sees only `list`.
    HostTool {
        tool: "case.request.send",
        group: "case",
        description: "ask an external party to quote, attend, confirm or inform: mints the link, \
                      emails it, schedules the nudges and sets waiting_on",
    },
    HostTool {
        tool: "case.request.withdraw",
        group: "case",
        description: "take an ask back — kills the link, cancels the nudges",
    },
    HostTool {
        tool: "case.request.nudge",
        group: "case",
        description: "fire one rung of an ask's nudge ladder (50 % / 80 % chase, breach escalation)",
    },
    HostTool {
        tool: "case.request.list",
        group: "case",
        description: "the asks raised on a case, with delivery reconciled against the outbox",
    },
    // The two token verbs. Listed because they are dispatched, and invisible in practice to every
    // logged-in caller: the catalog gate asks for their caps, which exist in no role bundle.
    HostTool {
        tool: "case.request.view",
        group: "case",
        description: "what a request link shows the party it was sent to (token principal only)",
    },
    HostTool {
        tool: "case.request.reply",
        group: "case",
        description: "the party's answer to one request (token principal only)",
    },
];
