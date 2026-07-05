//! The **static host-native tool catalog** — the authoritative list of built-in MCP verbs the host
//! dispatches directly (NOT components, so they have no manifest and are not in the runtime
//! `Registry`). `system.tools` appends this to the registry-derived extension tools so the catalog is
//! the *whole* reachable surface, not just the plugin half.
//!
//! It is kept beside the dispatcher it mirrors (`tool_call.rs::is_host_native`): every prefix that
//! file dispatches has at least one entry here (asserted by `host_catalog_covers_dispatch_prefixes`),
//! so a whole verb family cannot silently go missing from the console. The descriptions are
//! hand-written one-liners — source code is the only source of truth for a host verb (it has no
//! manifest to read), so the list lives here as a `const`.

use super::model::ToolInfo;

/// One static catalog row: the qualified verb, its group (the family prefix), and a one-line summary.
struct HostTool {
    tool: &'static str,
    group: &'static str,
    description: &'static str,
}

/// The built-in host-native verbs, grouped by family. Mirrors `tool_call.rs::is_host_native` (every
/// prefix there appears here) plus the host-native services that route outside that bridge (`system.*`
/// is dispatched by the gateway/UI directly, not the bridge, but it is still a reachable tool).
const HOST_TOOLS: &[HostTool] = &[
    // host.* — cross-platform node introspection (host-tools scope).
    HostTool {
        tool: "host.net.info",
        group: "host",
        description: "the node's hostname + network interfaces and their addresses",
    },
    HostTool {
        tool: "host.net.reach",
        group: "host",
        description: "test TCP reachability of a host:port from the node, with a timeout",
    },
    HostTool {
        tool: "host.time.now",
        group: "host",
        description: "the node's current UTC + local time, zone, and offset",
    },
    HostTool {
        tool: "host.time.zones",
        group: "host",
        description: "the time zones the node knows about",
    },
    HostTool {
        tool: "host.fs.stat",
        group: "host",
        description: "metadata for one path (exists, kind, size, mtime, permissions)",
    },
    HostTool {
        tool: "host.fs.list",
        group: "host",
        description: "a bounded directory listing with per-entry metadata",
    },
    // system.* — the read-only workspace topology + status console (system-map scope).
    HostTool {
        tool: "system.overview",
        group: "system",
        description: "the per-subsystem health + metrics status grid for the workspace",
    },
    HostTool {
        tool: "system.topology",
        group: "system",
        description: "nodes + wiring edges for the react-flow topology graph",
    },
    HostTool {
        tool: "system.subsystem",
        group: "system",
        description: "the full live detail of one subsystem (the no-page card drill-in)",
    },
    HostTool {
        tool: "system.tools",
        group: "system",
        description: "this catalog — every MCP tool reachable for the workspace, with descriptions",
    },
    HostTool {
        tool: "system.acp",
        group: "system",
        description: "the ACP adapter's static protocol/capability facts",
    },
    // agent.* — the central agent's policy/decision/run verbs (agent + agent-run scope).
    HostTool {
        tool: "agent.policy.set",
        group: "agent",
        description: "set the per-workspace autonomy policy the agent decides under",
    },
    HostTool {
        tool: "agent.decide",
        group: "agent",
        description: "record a decision on a suspended agent run (approve/deny/edit)",
    },
    HostTool {
        tool: "agent.watch",
        group: "agent",
        description: "subscribe to a run's RunEvent feed (the live turn projection)",
    },
    HostTool {
        tool: "agent.control",
        group: "agent",
        description: "stop / pause / resume a live agent run",
    },
    // bus.* — direct Zenoh bus introspection/publish over the host bridge (bus scope).
    HostTool {
        tool: "bus.publish",
        group: "bus",
        description: "publish a message onto a workspace-scoped bus subject",
    },
    HostTool {
        tool: "bus.peers",
        group: "bus",
        description: "the live peers/routers this node is connected to on the mesh",
    },
    // store.* — the read-only, parse-allowlisted SQL surface a widget/page reads (store-query scope).
    HostTool {
        tool: "store.query",
        group: "store",
        description: "a bounded, workspace-walled read-only SELECT over the embedded store",
    },
    HostTool {
        tool: "store.schema",
        group: "store",
        description: "the store schema (tables + columns) for the visual query builder",
    },
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
        tool: "insight.occurrences",
        group: "insight",
        description: "read the per-insight occurrence ring (newest-first, keyset-paged)",
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
    // dashboard.* — the grid-of-widgets surface verbs (dashboard scope).
    HostTool {
        tool: "dashboard.get",
        group: "dashboard",
        description: "read one dashboard by id",
    },
    HostTool {
        tool: "dashboard.list",
        group: "dashboard",
        description: "list the dashboards visible to the caller",
    },
    HostTool {
        tool: "dashboard.save",
        group: "dashboard",
        description: "create or update a dashboard the caller owns",
    },
    HostTool {
        tool: "dashboard.delete",
        group: "dashboard",
        description: "delete a dashboard the caller owns",
    },
    HostTool {
        tool: "dashboard.share",
        group: "dashboard",
        description: "share a dashboard with another principal/team",
    },
    // panel.* — the reusable + standalone library-panel asset (library-panels scope).
    HostTool {
        tool: "panel.get",
        group: "panel",
        description: "read one library panel by id (full spec)",
    },
    HostTool {
        tool: "panel.list",
        group: "panel",
        description: "list the library panels visible to the caller",
    },
    HostTool {
        tool: "panel.save",
        group: "panel",
        description: "create or update a library panel the caller owns",
    },
    HostTool {
        tool: "panel.delete",
        group: "panel",
        description: "delete a library panel the caller owns (refused while in use unless forced)",
    },
    HostTool {
        tool: "panel.share",
        group: "panel",
        description: "share a library panel with a team / set its visibility",
    },
    HostTool {
        tool: "panel.usage",
        group: "panel",
        description: "list the dashboards that reference a library panel",
    },
    // nav.* — the user-/team-authored navigation menu asset (nav scope).
    HostTool {
        tool: "nav.get",
        group: "nav",
        description: "read one navigation menu by id",
    },
    HostTool {
        tool: "nav.list",
        group: "nav",
        description: "list the navigation menus visible to the caller",
    },
    HostTool {
        tool: "nav.save",
        group: "nav",
        description: "create or update a navigation menu the caller owns",
    },
    HostTool {
        tool: "nav.delete",
        group: "nav",
        description: "delete a navigation menu the caller owns",
    },
    HostTool {
        tool: "nav.share",
        group: "nav",
        description: "share a navigation menu with a team / set its visibility",
    },
    HostTool {
        tool: "nav.set_default",
        group: "nav",
        description: "set the workspace-default navigation menu",
    },
    HostTool {
        tool: "nav.resolve",
        group: "nav",
        description: "resolve the caller's effective menu (picked, tag-expanded, cap-stripped)",
    },
    HostTool {
        tool: "nav.pref.get",
        group: "nav",
        description: "read the caller's own active-nav pick",
    },
    HostTool {
        tool: "nav.pref.set",
        group: "nav",
        description: "set the caller's own active-nav pick",
    },
    // template.* — the durable scripted-view snippets the widget builder persists (widget-builder scope).
    HostTool {
        tool: "template.get",
        group: "template",
        description: "read one render template (Plot/D3/JSX snippet) by id",
    },
    HostTool {
        tool: "template.list",
        group: "template",
        description: "list the render templates visible to the caller",
    },
    HostTool {
        tool: "template.save",
        group: "template",
        description: "create or update a render template the caller authors",
    },
    HostTool {
        tool: "template.delete",
        group: "template",
        description: "delete a render template the caller authors",
    },
    // devkit.* — the in-app extension scaffolding/build toolkit (devkit scope).
    HostTool {
        tool: "devkit.templates",
        group: "devkit",
        description: "list the extension scaffold templates available in Studio",
    },
    HostTool {
        tool: "devkit.root",
        group: "devkit",
        description: "the absolute devkit root directory the folder picker browses from",
    },
    HostTool {
        tool: "devkit.scaffold",
        group: "devkit",
        description: "scaffold a new extension from a template",
    },
    HostTool {
        tool: "devkit.inspect",
        group: "devkit",
        description: "inspect an extension's manifest + build inputs",
    },
    HostTool {
        tool: "devkit.build",
        group: "devkit",
        description: "build an extension's native sidecar + federated UI bundle",
    },
    // series.* / ingest.* — the generic ingest + read surface (ingest scope).
    HostTool {
        tool: "series.list",
        group: "series",
        description: "list the series (metrics) in the workspace",
    },
    HostTool {
        tool: "series.latest",
        group: "series",
        description: "the latest committed value of a series",
    },
    HostTool {
        tool: "series.find",
        group: "series",
        description: "find series by tag/name match",
    },
    HostTool {
        tool: "series.read",
        group: "series",
        description: "read a committed range of a series",
    },
    HostTool {
        tool: "ingest.write",
        group: "ingest",
        description: "write a sample into the exactly-once ingest buffer",
    },
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

/// The static host-native catalog as `ToolInfo` rows (`source = "host"`), sorted by qualified name so
/// the page renders a stable order. The extension half (registry-derived) is appended by the caller.
pub(crate) fn host_catalog() -> Vec<ToolInfo> {
    let mut out: Vec<ToolInfo> = HOST_TOOLS
        .iter()
        .map(|t| ToolInfo {
            tool: t.tool.to_string(),
            description: t.description.to_string(),
            source: "host".to_string(),
            group: t.group.to_string(),
        })
        .collect();
    out.sort_by(|a, b| a.tool.cmp(&b.tool));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every host-native verb-family prefix the dispatcher (`tool_call.rs::is_host_native`) routes has
    /// at least one catalog entry — so a whole family cannot silently vanish from the console. Mirrors
    /// the `||` arms of `is_host_native` (the bridge-dispatched families) plus `system.` (routed
    /// directly by the gateway/UI). `host.` is included; the non-prefixed `series.*`/`ingest.*` ingest
    /// fallthrough is covered by its own group.
    #[test]
    fn host_catalog_covers_dispatch_prefixes() {
        let cat = host_catalog();
        for prefix in [
            "series.",
            "ingest.",
            "outbox.",
            "inbox.",
            "dashboard.",
            "template.",
            "devkit.",
            "agent.",
            "host.",
            "bus.",
            "store.",
            "system.",
            "secret.",
        ] {
            assert!(
                cat.iter().any(|t| t.tool.starts_with(prefix)),
                "host catalog has no entry for dispatched prefix `{prefix}`"
            );
        }
    }

    #[test]
    fn every_row_is_well_formed() {
        for t in host_catalog() {
            assert!(!t.tool.is_empty(), "empty tool name");
            assert!(
                !t.description.is_empty(),
                "tool {} has no description",
                t.tool
            );
            assert_eq!(t.source, "host");
            assert!(!t.group.is_empty(), "tool {} has no group", t.tool);
        }
    }
}
