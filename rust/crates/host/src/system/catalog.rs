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
        description: "a bounded directory listing with per-entry metadata; optional name/extensions/include_hidden filters",
    },
    HostTool {
        tool: "host.fs.home",
        group: "host",
        description: "the node's home directory (a stable anchor to browse from)",
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
    // authz.* — the scoped read API (entity-scoped-grants scope) + the access-console verbs.
    HostTool {
        tool: "authz.check_scoped",
        group: "authz",
        description: "check if the calling principal may reach a record under a cap (entity-scoped)",
    },
    HostTool {
        tool: "authz.scope_filter",
        group: "authz",
        description: "which rows in a table the calling principal may reach under a cap",
    },
    HostTool {
        tool: "authz.delegate_reach",
        group: "authz",
        description: "marker cap: hold it to name a `subject` on check_scoped/scope_filter (delegated reach)",
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
    // media.* — the chunked-upload + variant + serve surface (media scope).
    HostTool {
        tool: "media.upload_begin",
        group: "media",
        description: "begin a resumable chunked upload (declares size/mime/checksum)",
    },
    HostTool {
        tool: "media.upload_commit",
        group: "media",
        description: "commit an upload (verify checksum, derive variants, flip to ready)",
    },
    HostTool {
        tool: "media.get",
        group: "media",
        description: "read media metadata by id",
    },
    HostTool {
        tool: "media.list",
        group: "media",
        description: "list media in the workspace",
    },
    HostTool {
        tool: "media.delete",
        group: "media",
        description: "archive media by id",
    },
    // device.* / notify.* — the push-notification surface (push-target scope).
    HostTool {
        tool: "device.register",
        group: "notify",
        description: "register a push device (self-only, upsert by token)",
    },
    HostTool {
        tool: "device.list",
        group: "notify",
        description: "list the caller's own registered devices",
    },
    HostTool {
        tool: "device.remove",
        group: "notify",
        description: "remove a registered device (self-only)",
    },
    HostTool {
        tool: "notify.send",
        group: "notify",
        description: "enqueue a push notification to an audience (outbox-delivered)",
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
        tool: "dashboard.delete_any",
        group: "dashboard",
        description:
            "admin override: delete any dashboard in the workspace, not just the caller's own",
    },
    HostTool {
        tool: "dashboard.share",
        group: "dashboard",
        description: "share a dashboard with another principal/team",
    },
    HostTool {
        tool: "dashboard.access_check",
        group: "dashboard",
        description:
            "read-only preflight: walk a dashboard's dependency closure and report, per dependency, whether a subject/team can render it (access-model scope)",
    },
    HostTool {
        tool: "dashboard.import",
        group: "dashboard",
        description:
            "import a Grafana dashboard JSON (preview returns a datasource-remap report; commit with mappings upserts a dashboard the caller owns)",
    },
    HostTool {
        tool: "dashboard.export",
        group: "dashboard",
        description: "export a dashboard the caller can read as Grafana JSON",
    },
    // identity.* — the credential-management admin verb (login-hardening scope). The directory
    // verbs (create/get/list/workspaces) also have dedicated admin REST routes.
    HostTool {
        tool: "identity.set_credential",
        group: "identity",
        description: "admin: set/rotate a user's login password (argon2-hashed; never returns the hash)",
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
    // report.* — the report-builder asset (reports scope).
    HostTool {
        tool: "report.get",
        group: "report",
        description: "read one report by id (blocks hydrated)",
    },
    HostTool {
        tool: "report.list",
        group: "report",
        description: "list the reports visible to the caller",
    },
    HostTool {
        tool: "report.save",
        group: "report",
        description: "create or update a report the caller owns",
    },
    HostTool {
        tool: "report.delete",
        group: "report",
        description: "delete a report the caller owns",
    },
    HostTool {
        tool: "report.share",
        group: "report",
        description: "share a report with a team / set its visibility",
    },
    HostTool {
        tool: "report.export",
        group: "report",
        description: "export a report to branded PDF (gateway binary route; own cap)",
    },
    // brand.* — the reusable brand-profile asset (reports scope).
    HostTool {
        tool: "brand.get",
        group: "brand",
        description: "read one brand profile by id",
    },
    HostTool {
        tool: "brand.list",
        group: "brand",
        description: "list the brand profiles in the workspace",
    },
    HostTool {
        tool: "brand.save",
        group: "brand",
        description: "create or update a brand profile the caller owns",
    },
    HostTool {
        tool: "brand.delete",
        group: "brand",
        description: "delete a brand profile the caller owns",
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
        description: "set the caller's own active-nav pick and/or pinned favorites",
    },
    // hide-and-pins scope: the workspace hidden-set — the admin's one subtractive sidebar-curation
    // lever (declutter only; hiding never blocks a route).
    HostTool {
        tool: "nav.hidden.get",
        group: "nav",
        description: "read the workspace sidebar hidden-set",
    },
    HostTool {
        tool: "nav.hidden.set",
        group: "nav",
        description: "replace the workspace sidebar hidden-set (admin)",
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
        tool: "devkit.write_file",
        group: "devkit",
        description: "write or replace a source file inside a scaffolded extension dir",
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
        description: "read a committed range of a series (keyset-paged rows or decimated buckets)",
    },
    HostTool {
        tool: "series.retention.set",
        group: "series",
        description: "set the retention policy for a series prefix: raw time horizon (raw_for_ms), \
                      FIFO sample cap (max_samples, 0 = unbounded), and rollup tiers",
    },
    HostTool {
        tool: "series.retention.list",
        group: "series",
        description: "list the workspace's series retention policies",
    },
    HostTool {
        tool: "series.retention.delete",
        group: "series",
        description: "delete a series retention policy (revert to keep-forever)",
    },
    HostTool {
        tool: "series.retention.gc",
        group: "series",
        description: "run one retention pass now: roll up then evict raw samples past the time \
                      horizon or over the sample cap (a reactor also ticks this on a cadence)",
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
    // datasource.* / federation.* — the external-datasource surface (datasources scope).
    HostTool {
        tool: "datasource.list",
        group: "datasource",
        description: "list the workspace's registered external datasources",
    },
    HostTool {
        tool: "datasource.test",
        group: "datasource",
        description: "test connectivity of a registered (or candidate) datasource",
    },
    HostTool {
        tool: "datasource.add",
        group: "datasource",
        description: "register an external datasource (DSN sealed into lb-secrets)",
    },
    HostTool {
        tool: "datasource.remove",
        group: "datasource",
        description: "remove a registered datasource",
    },
    HostTool {
        tool: "federation.query",
        group: "federation",
        description: "run SQL against one registered external datasource",
    },
    HostTool {
        tool: "federation.schema",
        group: "federation",
        description: "the tables + columns of one registered datasource",
    },
    HostTool {
        tool: "federation.mirror",
        group: "federation",
        description: "mirror an external query's rows into the embedded store",
    },
    HostTool {
        tool: "federation.write",
        group: "federation",
        description: "write rows to a registered datasource (bounded INSERT/UPSERT)",
    },
    HostTool {
        tool: "federation.migrate",
        group: "federation",
        description: "plan/apply a designed schema to a datasource (additive DDL, dry-run default)",
    },
    HostTool {
        tool: "federation.export",
        group: "federation",
        description: "export platform series data to an external datasource (durable job)",
    },
    HostTool {
        tool: "dbschema.save",
        group: "dbschema",
        description: "save a designed schema record (tables/columns/PK/FK + layout)",
    },
    HostTool {
        tool: "dbschema.get",
        group: "dbschema",
        description: "read one designed schema record (full, layout included)",
    },
    HostTool {
        tool: "dbschema.list",
        group: "dbschema",
        description: "list the workspace's designed schema records (name + table count)",
    },
    HostTool {
        tool: "dbschema.delete",
        group: "dbschema",
        description: "remove a designed schema record (tombstones — never touches a live DB)",
    },
    // viz.query — the ONE viz bridge (widget-platform scope).
    HostTool {
        tool: "viz.query",
        group: "viz",
        description: "run a saved/inline query shaped for charts + tables (the one viz bridge)",
    },
    // query.* — saved queries (query-workbench scope).
    HostTool {
        tool: "query.run",
        group: "query",
        description: "run a saved query by id (with optional parameter overrides)",
    },
    HostTool {
        tool: "query.save",
        group: "query",
        description: "save a named query definition",
    },
    HostTool {
        tool: "query.compile",
        group: "query",
        description: "compile a query definition to its target SQL without running it",
    },
    HostTool {
        tool: "query.get",
        group: "query",
        description: "read one saved query definition",
    },
    HostTool {
        tool: "query.list",
        group: "query",
        description: "list the workspace's saved queries",
    },
    HostTool {
        tool: "query.delete",
        group: "query",
        description: "delete a saved query",
    },
    // flows.* — the typed-node DAG engine (flows scope).
    HostTool {
        tool: "flows.save",
        group: "flows",
        description: "create or update a flow definition (nodes + wires)",
    },
    HostTool {
        tool: "flows.get",
        group: "flows",
        description: "read one flow definition",
    },
    HostTool {
        tool: "flows.list",
        group: "flows",
        description: "list the workspace's flows",
    },
    HostTool {
        tool: "flows.delete",
        group: "flows",
        description: "delete a flow",
    },
    HostTool {
        tool: "flows.enable",
        group: "flows",
        description: "enable/disable a flow's triggers",
    },
    HostTool {
        tool: "flows.run",
        group: "flows",
        description: "run a flow now (a manual run)",
    },
    HostTool {
        tool: "flows.inject",
        group: "flows",
        description: "inject a message into a flow node's port",
    },
    HostTool {
        tool: "flows.cancel",
        group: "flows",
        description: "cancel a running flow run",
    },
    HostTool {
        tool: "flows.suspend",
        group: "flows",
        description: "suspend a running flow run",
    },
    HostTool {
        tool: "flows.resume",
        group: "flows",
        description: "resume a suspended flow run",
    },
    HostTool {
        tool: "flows.watch",
        group: "flows",
        description: "watch a flow's live run events",
    },
    HostTool {
        tool: "flows.nodes",
        group: "flows",
        description: "the node-type catalog the flow canvas builds from",
    },
    HostTool {
        tool: "flows.node.get",
        group: "flows",
        description: "read one node of a flow definition",
    },
    HostTool {
        tool: "flows.node.update",
        group: "flows",
        description: "update one node of a flow definition",
    },
    HostTool {
        tool: "flows.node_state",
        group: "flows",
        description: "the per-node live runtime value (the canvas steady-state view)",
    },
    HostTool {
        tool: "flows.patch_run",
        group: "flows",
        description: "patch a suspended run's pending state before resuming",
    },
    HostTool {
        tool: "flows.runs.get",
        group: "flows",
        description: "read one flow run's record",
    },
    HostTool {
        tool: "flows.runs.list",
        group: "flows",
        description: "list a flow's runs",
    },
    // rules.* — rule authoring + evaluation (rules-workbench scope).
    HostTool {
        tool: "rules.save",
        group: "rules",
        description: "create or update a rule",
    },
    HostTool {
        tool: "rules.get",
        group: "rules",
        description: "read one rule",
    },
    HostTool {
        tool: "rules.list",
        group: "rules",
        description: "list the workspace's rules",
    },
    HostTool {
        tool: "rules.delete",
        group: "rules",
        description: "delete a rule",
    },
    HostTool {
        tool: "rules.run",
        group: "rules",
        description: "run a rule now against real data",
    },
    HostTool {
        tool: "rules.eval",
        group: "rules",
        description: "evaluate a rule expression without saving it",
    },
    HostTool {
        tool: "rules.help",
        group: "rules",
        description: "the rule grammar + function reference",
    },
    // channel.* — the host's messaging plane (rules-messaging scope).
    HostTool {
        tool: "channel.create",
        group: "channel",
        description: "register a channel so it is listable before the first post (bus pub cap re-checked)",
    },
    HostTool {
        tool: "channel.post",
        group: "channel",
        description: "post a message to a channel (bus cap re-checked per channel)",
    },
    HostTool {
        tool: "channel.list",
        group: "channel",
        description: "list the workspace's channels",
    },
    HostTool {
        tool: "channel.history",
        group: "channel",
        description: "read a channel's persisted message history",
    },
    HostTool {
        tool: "channel.edit",
        group: "channel",
        description: "edit one of your own channel messages",
    },
    HostTool {
        tool: "channel.delete",
        group: "channel",
        description: "delete one of your own channel messages",
    },
    HostTool {
        tool: "channel.chart_pref.get",
        group: "channel",
        description: "read your per-viewer chart preference for a query result",
    },
    HostTool {
        tool: "channel.chart_pref.set",
        group: "channel",
        description: "set your per-viewer chart preference for a query result",
    },
    // prefs.* — member/workspace preference axes (prefs scope).
    HostTool {
        tool: "prefs.get",
        group: "prefs",
        description: "read your raw member preference record",
    },
    HostTool {
        tool: "prefs.set",
        group: "prefs",
        description: "set one of your member preference axes",
    },
    HostTool {
        tool: "prefs.resolve",
        group: "prefs",
        description: "your effective preferences (member over workspace defaults)",
    },
    HostTool {
        tool: "prefs.set_default",
        group: "prefs",
        description: "set a workspace-default preference axis (admin)",
    },
    HostTool {
        tool: "prefs.catalog",
        group: "prefs",
        description: "the preference axes + allowed values",
    },
    // message.* — recipient-localized rendering (i18n-catalogs scope).
    HostTool {
        tool: "message.render",
        group: "message",
        description: "render a message template with values (caller's locale)",
    },
    HostTool {
        tool: "message.render_recipient",
        group: "message",
        description: "render a message template localized to another recipient",
    },
    HostTool {
        tool: "message.set_catalog",
        group: "message",
        description: "write a message-template catalog (admin)",
    },
    // reminder.* — scheduled reminders (reminders-tenant scope).
    HostTool {
        tool: "reminder.create",
        group: "reminder",
        description: "create a scheduled reminder",
    },
    HostTool {
        tool: "reminder.update",
        group: "reminder",
        description: "update a reminder",
    },
    HostTool {
        tool: "reminder.delete",
        group: "reminder",
        description: "delete a reminder",
    },
    HostTool {
        tool: "reminder.list",
        group: "reminder",
        description: "list the workspace's reminders",
    },
    HostTool {
        tool: "reminder.fire",
        group: "reminder",
        description: "fire a reminder now (the gated run-now control)",
    },
    // assets.* — docs, skills, and binary assets (assets scope).
    HostTool {
        tool: "assets.list_docs",
        group: "assets",
        description: "list the workspace's shared docs",
    },
    HostTool {
        tool: "assets.get_doc",
        group: "assets",
        description: "read one shared doc",
    },
    HostTool {
        tool: "assets.link_doc",
        group: "assets",
        description: "link a doc to another record",
    },
    HostTool {
        tool: "assets.delete_doc",
        group: "assets",
        description: "delete a shared doc",
    },
    HostTool {
        tool: "assets.list_assets",
        group: "assets",
        description: "list the workspace's binary assets",
    },
    HostTool {
        tool: "assets.get_asset",
        group: "assets",
        description: "read one binary asset's metadata",
    },
    HostTool {
        tool: "assets.delete_asset",
        group: "assets",
        description: "delete a binary asset",
    },
    HostTool {
        tool: "assets.backlinks",
        group: "assets",
        description: "the records linking to a doc/asset",
    },
    HostTool {
        tool: "assets.list_granted_skills",
        group: "assets",
        description: "list the skills granted to you",
    },
    HostTool {
        tool: "assets.load_skill",
        group: "assets",
        description: "load a granted skill's body (grant-gated)",
    },
    // docs.* — doc-derived operations (doc-extraction scope; embeddings scope adds search/reindex).
    HostTool {
        tool: "docs.extract",
        group: "docs",
        description: "derive markdown docs from binary media (PDF/XLSX/CSV/HTML/text)",
    },
    // telemetry.* — the redacted dispatch/telemetry log (observability scope).
    HostTool {
        tool: "telemetry.query",
        group: "telemetry",
        description: "query the redacted telemetry log",
    },
    HostTool {
        tool: "telemetry.tail",
        group: "telemetry",
        description: "tail recent telemetry events",
    },
    HostTool {
        tool: "telemetry.trace",
        group: "telemetry",
        description: "the events of one trace id",
    },
    HostTool {
        tool: "telemetry.purge",
        group: "telemetry",
        description: "purge telemetry rows (admin)",
    },
    // layout.* — the member-owned per-surface layout record (data-studio v2 scope).
    HostTool {
        tool: "layout.get",
        group: "layout",
        description: "read your saved layout for a surface",
    },
    HostTool {
        tool: "layout.set",
        group: "layout",
        description: "save your layout for a surface",
    },
    // history/undo — the compensation log (undo scope).
    HostTool {
        tool: "history.list",
        group: "history",
        description: "list the workspace's mutation history",
    },
    HostTool {
        tool: "history.compensations",
        group: "history",
        description: "the compensations available for a history entry",
    },
    HostTool {
        tool: "undo",
        group: "history",
        description: "undo your latest undoable mutation",
    },
    HostTool {
        tool: "redo",
        group: "history",
        description: "redo your latest undone mutation",
    },
    // store.* write half (the read half is above).
    HostTool {
        tool: "store.write",
        group: "store",
        description: "write one record into the embedded store",
    },
    HostTool {
        tool: "store.delete",
        group: "store",
        description: "delete one record from the embedded store",
    },
    // tools.* — the palette/agent menu source itself.
    HostTool {
        tool: "tools.catalog",
        group: "tools",
        description: "the MCP tools you are authorized to call in this workspace",
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
    /// at least one catalog entry — so a whole family cannot silently vanish from the console OR the
    /// agent's `tools.catalog`-derived menu (which now serves this inventory). Derived from the
    /// dispatcher's OWN shared const, not a hand-copied list — a hand-maintained mirror is exactly how
    /// `datasource.`/`viz.`/`flows.`/… went missing (see
    /// debugging/agent/persona-menu-missing-tools-catalog-descriptor-only.md). `system.` (routed
    /// directly by the gateway/UI, not the bridge) is asserted on top.
    #[test]
    fn host_catalog_covers_dispatch_prefixes() {
        let cat = host_catalog();
        for prefix in crate::tool_call::HOST_NATIVE_PREFIXES
            .iter()
            .chain(["system."].iter())
        {
            assert!(
                cat.iter().any(|t| t.tool.starts_with(prefix)),
                "host catalog has no entry for dispatched prefix `{prefix}`"
            );
        }
        for exact in crate::tool_call::HOST_NATIVE_EXACT {
            assert!(
                cat.iter().any(|t| &t.tool == exact),
                "host catalog has no entry for dispatched verb `{exact}`"
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
