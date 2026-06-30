// The real HTTP transport to a node's SSE/HTTP gateway. The same command verbs the feature code
// calls (`channel_post`, `inbox_list`, …) map onto the gateway's REST routes one-to-one, and every
// request carries the session bearer token (collaboration scope, slice 1) — so the gateway derives
// the principal + workspace from the token (the hard wall, §7), never from the request body.
//
// One verb per command, mapped to the gateway routes in `role/gateway`:
//   login            → POST /login                         (no token — it issues one)
//   workspace_list   → GET  /workspaces
//   workspace_create → POST /workspaces
//   channel_list     → GET  /channels
//   channel_create   → POST /channels
//   channel_post     → POST   /channels/{cid}/messages
//   channel_history  → GET    /channels/{cid}/messages
//   channel_edit     → PATCH  /channels/{cid}/messages/{id}
//   channel_delete   → DELETE /channels/{cid}/messages/{id}
//   members_list     → GET  /teams/{team}/members
//   members_add      → POST /teams/{team}/members
//   inbox_list       → GET  /inbox/{channel}
//   inbox_resolve    → POST /inbox/{item}/resolve
//   outbox_status    → GET  /outbox
//
// The base URL comes from `VITE_GATEWAY_URL` (the browser build). The feature code never sees this —
// it goes through `invoke`, exactly as it does for Tauri and the fake.

import type { Item } from "@/lib/channel/channel.types";
import { sessionToken } from "@/lib/session/session.store";

/** The gateway base URL, e.g. `http://127.0.0.1:8080`. In the browser this defaults to the local dev
 *  node so the app always talks to a REAL gateway out of the box (override with `VITE_GATEWAY_URL`;
 *  the real-gateway test harness stubs it to the spawned node's URL). Empty only when there is no
 *  `window` (a non-browser context that isn't the harness) — `invoke` then throws rather than fake. */
export function gatewayUrl(): string {
  const configured = import.meta.env.VITE_GATEWAY_URL as string | undefined;
  if (configured !== undefined) return configured;
  // Browser with no explicit config → the local dev gateway.
  if (typeof window !== "undefined") return "http://127.0.0.1:8080";
  return "";
}

/** The Authorization header for the current session, or none when logged out. */
function authHeaders(): Record<string, string> {
  const token = sessionToken();
  return token ? { authorization: `Bearer ${token}` } : {};
}

const enc = encodeURIComponent;

/** The node's `Effect` wire shape (a subset) — flattened into the workflow `Effect` view by
 *  `workflow_list_effects` (the node has no per-workflow list; the outbox is the durable truth). */
interface RawEffect {
  target: string;
  action: string;
  idempotency_key: string;
  status: string;
}

export async function httpInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const base = gatewayUrl();
  switch (cmd) {
    case "login": {
      const { user, workspace } = args as { user: string; workspace: string };
      return postJson<T>(`${base}/login`, { user, workspace }, /* auth */ false);
    }
    case "workspace_list":
      return getJson<T>(`${base}/workspaces`);
    case "workspace_create": {
      const { ws, name } = args as { ws: string; name: string };
      return postJson<T>(`${base}/workspaces`, { ws, name });
    }
    case "channel_list":
      return getJson<T>(`${base}/channels`);
    case "channel_create": {
      const { channel } = args as { channel: string };
      return postJson<T>(`${base}/channels`, { channel });
    }
    case "channel_post": {
      const { channel, item } = args as { channel: string; item: Item };
      return postJson<T>(`${base}/channels/${enc(channel)}/messages`, item);
    }
    case "channel_history": {
      const { channel } = args as { channel: string };
      return getJson<T>(`${base}/channels/${enc(channel)}/messages`);
    }
    case "channel_edit": {
      // Edit the body of one of the caller's own messages. The id + channel are in the path; the
      // new body + logical ts ride in the PATCH body. The host re-checks author ownership.
      const { channel, id, body, ts } = args as {
        channel: string;
        id: string;
        body: string;
        ts: number;
      };
      return patchJson<T>(`${base}/channels/${enc(channel)}/messages/${enc(id)}`, { body, ts });
    }
    case "channel_delete": {
      const { channel, id } = args as { channel: string; id: string };
      return delJson<T>(`${base}/channels/${enc(channel)}/messages/${enc(id)}`);
    }
    case "members_list": {
      const { team } = args as { team: string };
      return getJson<T>(`${base}/teams/${enc(team)}/members`);
    }
    case "members_add": {
      const { team, user } = args as { team: string; user: string };
      return postJson<T>(`${base}/teams/${enc(team)}/members`, { user });
    }
    case "inbox_list": {
      const { channel } = args as { channel: string };
      return getJson<T>(`${base}/inbox/${enc(channel)}`);
    }
    case "inbox_resolve": {
      const { item, decision } = args as { item: string; decision: string };
      return postJson<T>(`${base}/inbox/${enc(item)}/resolve`, { decision });
    }
    case "outbox_status":
      return getJson<T>(`${base}/outbox`);

    // ── admin-crud: the destructive/admin surface (admin-console scope). Each maps 1:1 to an
    //    /admin/* (or members) gateway route; the gateway re-checks the capability server-side,
    //    so the UI cap-gate is convenience only. No more `unknown command` in the browser. ──
    case "members_remove": {
      const { team, user } = args as { team: string; user: string };
      return delJson<T>(`${base}/teams/${enc(team)}/members/${enc(user)}`);
    }
    case "user_list":
      return getJson<T>(`${base}/admin/users`);
    case "user_create": {
      const { user, role } = args as { user: string; role?: string };
      return postJson<T>(`${base}/admin/users`, { user, role });
    }
    case "user_disable": {
      const { user } = args as { user: string };
      return postJson<T>(`${base}/admin/users/${enc(user)}/disable`, {});
    }
    case "user_enable": {
      const { user } = args as { user: string };
      return postJson<T>(`${base}/admin/users/${enc(user)}/enable`, {});
    }
    case "user_delete": {
      const { user } = args as { user: string };
      return delJson<T>(`${base}/admin/users/${enc(user)}`);
    }
    case "teams_list":
      return getJson<T>(`${base}/admin/teams`);
    case "teams_create": {
      const { team, name } = args as { team: string; name: string };
      return postJson<T>(`${base}/admin/teams`, { team, name });
    }
    case "teams_rename": {
      const { team, name } = args as { team: string; name: string };
      return postJson<T>(`${base}/admin/teams/${enc(team)}/rename`, { name });
    }
    case "teams_delete": {
      const { team } = args as { team: string };
      return delJson<T>(`${base}/admin/teams/${enc(team)}`);
    }
    case "workspace_rename": {
      const { ws, name } = args as { ws: string; name: string };
      return postJson<T>(`${base}/admin/workspaces/${enc(ws)}/rename`, { name });
    }
    case "workspace_archive": {
      const { ws } = args as { ws: string };
      return postJson<T>(`${base}/admin/workspaces/${enc(ws)}/archive`, {});
    }
    case "workspace_purge": {
      const { ws, confirm } = args as { ws: string; confirm: string };
      return postJson<T>(`${base}/admin/workspaces/${enc(ws)}/purge`, { confirm });
    }
    case "grants_list": {
      const { subject } = args as { subject: string };
      return getJson<T>(`${base}/admin/grants?subject=${enc(subject)}`);
    }
    case "grants_assign": {
      const { subject, cap } = args as { subject: string; cap: string };
      return postJson<T>(`${base}/admin/grants`, { subject, cap });
    }
    case "grants_revoke": {
      const { subject, cap } = args as { subject: string; cap: string };
      return postJson<T>(`${base}/admin/grants/revoke`, { subject, cap });
    }
    case "roles_list":
      return getJson<T>(`${base}/admin/roles`);
    case "roles_define": {
      const { name, caps } = args as { name: string; caps: string[] };
      return postJson<T>(`${base}/admin/roles`, { name, caps });
    }
    // ── access-console scope — the access-graph gaps: resolved effective caps WITH provenance, the
    //    live-token revoke lever (composes with grant-revoke), and roles.delete cascade. Each re-checks
    //    its admin cap server-side; ws + principal from the token. ──
    case "authz_resolve": {
      const { subject } = args as { subject: string };
      return getJson<T>(`${base}/admin/authz/resolve?subject=${enc(subject)}`);
    }
    case "authz_revoke_tokens": {
      const { subject } = args as { subject: string };
      return postJson<T>(`${base}/admin/authz/revoke-tokens`, { subject });
    }
    case "roles_delete": {
      const { name } = args as { name: string };
      return delJson<T>(`${base}/admin/roles/${enc(name)}`);
    }
    // ── global-identity scope — the global identity directory + per-workspace membership roster.
    //    The People tab reads `membership_list`; the switcher reads `identity_workspaces`. Each
    //    re-checks its cap server-side; ws + principal from the token. ──
    case "identity_list":
      return getJson<T>(`${base}/admin/identities`);
    case "identity_create": {
      const { sub, display_name } = args as { sub: string; display_name?: string };
      return postJson<T>(`${base}/admin/identities`, { sub, display_name });
    }
    case "identity_get": {
      const { sub } = args as { sub: string };
      return getJson<T>(`${base}/admin/identities/${enc(sub)}`);
    }
    case "identity_workspaces": {
      const { sub } = args as { sub: string };
      return getJson<T>(`${base}/admin/identities/${enc(sub)}/workspaces`);
    }
    case "membership_list":
      return getJson<T>(`${base}/admin/members`);
    case "membership_add": {
      const { sub } = args as { sub: string };
      return postJson<T>(`${base}/admin/members`, { sub });
    }
    case "membership_remove": {
      const { sub } = args as { sub: string };
      return delJson<T>(`${base}/admin/members/${enc(sub)}`);
    }
    case "apikey_list":
      return getJson<T>(`${base}/admin/apikeys`);
    case "apikey_create": {
      const { label, kind, role, caps, expires_at } = args as {
        label: string;
        kind?: string;
        role?: string;
        caps?: string[];
        expires_at?: number;
      };
      return postJson<T>(`${base}/admin/apikeys`, { label, kind, role, caps, expires_at });
    }
    case "apikey_get": {
      const { id } = args as { id: string };
      return getJson<T>(`${base}/admin/apikeys/${enc(id)}`);
    }
    case "apikey_revoke": {
      const { id } = args as { id: string };
      return postJson<T>(`${base}/admin/apikeys/${enc(id)}/revoke`, {});
    }
    case "apikey_rotate": {
      const { id } = args as { id: string };
      return postJson<T>(`${base}/admin/apikeys/${enc(id)}/rotate`, {});
    }

    // ── extension lifecycle (lifecycle-management scope): the browser's `ext.*` surface, finally
    //    reachable over the gateway (was Tauri-desktop-only → `unknown command` in the browser). ──
    case "ext_list":
      return getJson<T>(`${base}/extensions`);
    case "ext_enable": {
      const { ext } = args as { ext: string };
      return postJson<T>(`${base}/extensions/${enc(ext)}/enable`, {});
    }
    case "ext_disable": {
      const { ext } = args as { ext: string };
      return postJson<T>(`${base}/extensions/${enc(ext)}/disable`, {});
    }
    case "ext_uninstall": {
      const { ext } = args as { ext: string };
      return delJson<T>(`${base}/extensions/${enc(ext)}`);
    }
    case "ext_publish": {
      // Upload a signed extension artifact. The body is the `Artifact` verbatim (the same wire shape
      // the host's `ext_publish` / the registry-host `POST /artifacts` accept); the gateway derives the
      // workspace from the token and verify-before-stores. `204` ok / `422` verification failure.
      const { artifact } = args as { artifact: unknown };
      return postJson<T>(`${base}/extensions`, artifact as Record<string, unknown>);
    }

    // ── the host-mediated bridge endpoint (ui-federation scope): an extension page/widget reaches a
    //    granted MCP tool through here. The shell holds the token; the host re-checks cap + workspace.
    case "mcp_call": {
      const { tool, args: toolArgs } = args as { tool: string; args?: unknown };
      return postJson<T>(`${base}/mcp/call`, { tool, args: toolArgs ?? {} });
    }

    // ── shared assets (files/skills scope): the browser's `assets.*` surface over the gateway (was
    //    Tauri-only → `unknown command` in the browser). The `ws`/`author` the fake passes are
    //    DROPPED here — the gateway derives both from the token (the hard wall, §7). The host
    //    re-runs the S4 gates server-side. ──
    case "assets_put_doc": {
      const { id, title, content } = args as Record<string, string>;
      return postJson<T>(`${base}/docs`, { id, title, content });
    }
    case "assets_get_doc": {
      const { id } = args as Record<string, string>;
      return getJson<T>(`${base}/docs/${enc(id)}`);
    }
    case "assets_list_docs":
      return getJson<T>(`${base}/docs`);
    case "assets_share_doc": {
      const { id, team } = args as Record<string, string>;
      return postJson<T>(`${base}/docs/${enc(id)}/share`, { team });
    }
    case "assets_link_doc": {
      const { id, channel } = args as Record<string, string>;
      return postJson<T>(`${base}/docs/${enc(id)}/link`, { channel });
    }
    case "assets_put_skill": {
      const { id, version, description, body } = args as Record<string, string>;
      return postJson<T>(`${base}/skills`, { id, version, description: description ?? "", body });
    }
    case "assets_grant_skill": {
      const { id } = args as Record<string, string>;
      return postJson<T>(`${base}/skills/${enc(id)}/grant`, {});
    }
    case "assets_load_skill": {
      const { id, version } = args as Record<string, string>;
      const q = version ? `?version=${enc(version)}` : "";
      return getJson<T>(`${base}/skills/${enc(id)}${q}`);
    }

    // ── coding workflow (coding-workflow scope): the browser's `workflow.*` surface over the
    //    gateway. The PR coordinates are recorded at `request` and read back by `start` (no PR args
    //    on the start wire). The headline S6 approval gate runs server-side. ──
    case "workflow_request_approval": {
      const { approvalId, scopeDoc, team, pr } = args as {
        approvalId: string;
        scopeDoc: string;
        team: string;
        pr: Record<string, unknown>;
      };
      return postJson<T>(`${base}/approvals/${enc(approvalId)}/request`, {
        scope_doc: scopeDoc,
        team,
        pr,
      });
    }
    case "workflow_resolve_approval": {
      const { approvalId, decision } = args as { approvalId: string; decision: string };
      return postJson<T>(`${base}/approvals/${enc(approvalId)}/resolve`, { decision });
    }
    case "workflow_start_job": {
      const { approvalId, jobId, scopeDoc, channel, prKey } = args as {
        approvalId: string;
        jobId: string;
        scopeDoc: string;
        channel: string;
        prKey: string;
      };
      return postJson<T>(`${base}/approvals/${enc(approvalId)}/start`, {
        job_id: jobId,
        scope_doc: scopeDoc,
        channel,
        pr_key: prKey,
      });
    }
    case "workflow_list_effects": {
      // The real node has no per-workflow effect list — the durable truth is the workspace outbox.
      // Map onto `GET /outbox` and flatten the lifecycle groups into the workflow `Effect[]` shape
      // the UI renders (target/action/idempotencyKey/status). `dead-lettered` collapses to `failed`.
      const status = await getJson<{
        pending: RawEffect[];
        delivered: RawEffect[];
        dead_lettered: RawEffect[];
      }>(`${base}/outbox`);
      const all = [...status.pending, ...status.delivered, ...status.dead_lettered];
      return all.map((e) => ({
        target: e.target,
        action: e.action,
        idempotencyKey: e.idempotency_key,
        status: e.status === "delivered" ? "delivered" : e.status === "pending" ? "pending" : "failed",
      })) as T;
    }

    // ── DB browser (data-console scope): the browser's admin, READ-ONLY `store.*` lens. Each maps
    //    1:1 to a `/store/*` route; the gateway re-checks the **admin** cap server-side (these verbs
    //    relax gate 3, so they are admin-only). No write commands by design. ──
    case "store_tables":
      return getJson<T>(`${base}/store/tables`);
    case "store_scan": {
      const { table, limit, cursor } = args as {
        table: string;
        limit?: number;
        cursor?: string;
      };
      const q = new URLSearchParams();
      if (limit !== undefined) q.set("limit", String(limit));
      if (cursor) q.set("cursor", cursor);
      const qs = q.toString();
      return getJson<T>(`${base}/store/tables/${enc(table)}/rows${qs ? `?${qs}` : ""}`);
    }
    case "store_graph": {
      const { table, id, depth } = args as { table?: string; id?: string; depth?: number };
      const q = new URLSearchParams();
      if (table) q.set("table", table);
      if (id) q.set("id", id);
      if (depth !== undefined) q.set("depth", String(depth));
      const qs = q.toString();
      return getJson<T>(`${base}/store/graph${qs ? `?${qs}` : ""}`);
    }

    // ── System map (system-map scope): the browser's admin, READ-ONLY `system.*` lens. Each maps
    //    1:1 to a `/system/*` route; the gateway re-checks the **admin** cap server-side. The
    //    workspace comes from the token, never the request. No write commands by design. ──
    case "system_overview":
      return getJson<T>(`${base}/system/overview`);
    case "system_topology":
      return getJson<T>(`${base}/system/topology`);
    case "system_subsystem": {
      const { id } = args as { id: string };
      return getJson<T>(`${base}/system/subsystem/${enc(id)}`);
    }
    case "system_tools":
      return getJson<T>(`${base}/system/tools`);
    case "system_acp":
      return getJson<T>(`${base}/system/acp`);

    // ── ingest / series (data-console scope): the browser's `ingest.*`/`series.*` surface (the S8
    //    verbs over the gateway). The producer is the token's principal (un-spoofable); the write
    //    route drains the workspace so a manual sample is visible on the next read. ──
    case "ingest_write": {
      const { samples } = args as { samples: unknown[] };
      return postJson<T>(`${base}/ingest`, { samples });
    }
    case "series_list": {
      const { prefix } = args as { prefix?: string };
      const qs = prefix ? `?prefix=${enc(prefix)}` : "";
      return getJson<T>(`${base}/series${qs}`);
    }
    case "series_find": {
      const { facets } = args as { facets: unknown[] };
      return postJson<T>(`${base}/series/find`, { facets });
    }
    case "series_latest": {
      const { series } = args as { series: string };
      return getJson<T>(`${base}/series/${enc(series)}/latest`);
    }
    case "series_read": {
      const { series, from, to } = args as { series: string; from?: number; to?: number };
      const q = new URLSearchParams();
      if (from !== undefined) q.set("from", String(from));
      if (to !== undefined) q.set("to", String(to));
      const qs = q.toString();
      return getJson<T>(`${base}/series/${enc(series)}/samples${qs ? `?${qs}` : ""}`);
    }

    // ── dashboard (dashboard scope): the browser's `dashboard.*` CRUD over the gateway. The owner +
    //    workspace come from the token (§7); visibility is set via the `/share` route, never on save.
    //    The gateway re-checks the three gates (workspace → cap → membership/visibility) server-side. ──
    case "dashboard_list":
      return getJson<T>(`${base}/dashboards`);
    case "dashboard_get": {
      const { id } = args as { id: string };
      return getJson<T>(`${base}/dashboards/${enc(id)}`);
    }
    case "dashboard_save": {
      // `variables` is additive (widget-config-vars Slice 2) — forwarded so the bar + interpolation
      // round-trip; a pre-variables caller omits it (the gateway defaults it to []).
      const { id, title, cells, variables } = args as {
        id: string;
        title: string;
        cells: unknown[];
        variables?: unknown[];
      };
      return postJson<T>(`${base}/dashboards`, { id, title, cells, variables: variables ?? [] });
    }
    case "dashboard_delete": {
      const { id } = args as { id: string };
      return delJson<T>(`${base}/dashboards/${enc(id)}`);
    }
    case "dashboard_share": {
      const { id, visibility, team } = args as { id: string; visibility: string; team?: string };
      return postJson<T>(`${base}/dashboards/${enc(id)}/share`, { visibility, team });
    }

    // ── datasources (rules-workbench scope, Phase 3): the browser's `datasource.*` admin surface over
    //    the gateway. Each maps 1:1 to a `/datasources` route; the gateway re-checks
    //    `mcp:datasource.<verb>:call` server-side. The DSN is sent ONLY on `datasource_add` and never
    //    returned by any response (the redaction rule, §6.7). ──
    case "datasource_list":
      return getJson<T>(`${base}/datasources`);
    case "datasource_add": {
      const { name, kind, endpoint, dsn } = args as {
        name: string;
        kind: string;
        endpoint: string;
        dsn: string;
      };
      return postJson<T>(`${base}/datasources`, { name, kind, endpoint, dsn });
    }
    case "datasource_remove": {
      const { name } = args as { name: string };
      return delJson<T>(`${base}/datasources/${enc(name)}`);
    }
    case "datasource_test": {
      const { name } = args as { name: string };
      return postJson<T>(`${base}/datasources/${enc(name)}/test`, {});
    }

    // ── chains (rules-workbench scope, Phase 2): the browser's `chains.*` DAG-canvas CRUD + run +
    //    the per-step run snapshot poll. Each maps 1:1 to a `/chains/*` route; the gateway re-checks
    //    `mcp:chains.<verb>:call` server-side and sets the workspace from the token (§7). The `chain`
    //    body is POSTed verbatim (minus workspace, which the gateway injects). ──
    case "chains_list":
      return getJson<T>(`${base}/chains`);
    case "chains_get": {
      const { id } = args as { id: string };
      return getJson<T>(`${base}/chains/${enc(id)}`);
    }
    case "chains_save": {
      const { chain } = args as { chain: Record<string, unknown> };
      return postJson<T>(`${base}/chains`, chain);
    }
    case "chains_delete": {
      const { id } = args as { id: string };
      return delJson<T>(`${base}/chains/${enc(id)}`);
    }
    case "chains_run": {
      const { id, params } = args as { id: string; params?: unknown };
      return postJson<T>(`${base}/chains/${enc(id)}/run`, { params: params ?? {} });
    }
    case "chains_run_get": {
      const { id, runId } = args as { id: string; runId: string };
      return getJson<T>(`${base}/chains/${enc(id)}/runs/${enc(runId)}`);
    }

    // ── flows (flows-canvas + dashboard-binding scopes, Wave 3): the browser's `flows.*` typed-node
    //    canvas CRUD + run + the per-node run snapshot + reattach + enable/inject. Each maps 1:1 to a
    //    `/flows/*` route; the gateway re-checks `mcp:flows.<verb>:call` server-side and sets the
    //    workspace from the token (§7). The `flow` body is POSTed verbatim (minus workspace, which the
    //    gateway injects). `flows.inject` is the one write tool a dashboard control calls. ──
    case "flows_nodes":
      return getJson<T>(`${base}/flows/nodes`);
    case "flows_list":
      return getJson<T>(`${base}/flows`);
    case "flows_get": {
      const { id } = args as { id: string };
      return getJson<T>(`${base}/flows/${enc(id)}`);
    }
    case "flows_save": {
      const { flow } = args as { flow: Record<string, unknown> };
      return postJson<T>(`${base}/flows`, flow);
    }
    case "flows_delete": {
      const { id } = args as { id: string };
      return delJson<T>(`${base}/flows/${enc(id)}`);
    }
    case "flows_run": {
      const { id, params } = args as { id: string; params?: unknown };
      return postJson<T>(`${base}/flows/${enc(id)}/run`, { params: params ?? {} });
    }
    case "flows_suspend": {
      const { runId } = args as { runId: string };
      return postJson<T>(`${base}/flows/runs/${enc(runId)}/suspend`, {});
    }
    case "flows_resume": {
      const { runId } = args as { runId: string };
      return postJson<T>(`${base}/flows/runs/${enc(runId)}/resume`, {});
    }
    case "flows_cancel": {
      const { runId } = args as { runId: string };
      return postJson<T>(`${base}/flows/runs/${enc(runId)}/cancel`, {});
    }
    case "flows_patch_run": {
      const { runId, node, config } = args as {
        runId: string;
        node: string;
        config: unknown;
      };
      return postJson<T>(`${base}/flows/runs/${enc(runId)}/patch`, { node, config });
    }
    case "flows_run_get": {
      const { runId } = args as { runId: string };
      return getJson<T>(`${base}/flows/runs/${enc(runId)}`);
    }
    case "flows_runs_list": {
      const { flowId, status } = args as { flowId: string; status?: string | null };
      const qs = status ? `?status=${enc(status)}` : "";
      return getJson<T>(`${base}/flows/${enc(flowId)}/runs${qs}`);
    }
    case "flows_enable": {
      const { id, enabled, startOnBoot } = args as {
        id: string;
        enabled: boolean;
        startOnBoot: boolean;
      };
      return postJson<T>(`${base}/flows/${enc(id)}/enable`, {
        enabled,
        start_on_boot: startOnBoot,
      });
    }
    case "flows_inject": {
      const { id, node, value } = args as {
        id: string;
        node: string;
        value: unknown;
      };
      return postJson<T>(`${base}/flows/${enc(id)}/inject`, { node, value });
    }

    // ── rules (rules-workbench scope, Phase 1): the browser's `rules.*` Playground CRUD + run over
    //    the gateway. The workspace + principal come from the token (§7); each route re-checks
    //    `mcp:rules.<verb>:call` server-side. A 403 body is a generic "not permitted"; a 400 body is the
    //    verbatim author feedback (a cage/parse error, an AI-budget / AI-not-configured message). ──
    case "rules_run": {
      const { body, rule_id, params } = args as {
        body?: string;
        rule_id?: string;
        params?: Record<string, unknown>;
      };
      return postJson<T>(`${base}/rules/run`, { body, rule_id, params: params ?? {} });
    }
    case "rules_save": {
      const { id, name, body, params } = args as {
        id: string;
        name?: string;
        body: string;
        params?: unknown[];
      };
      return postJson<T>(`${base}/rules`, { id, name, body, params: params ?? [] });
    }
    case "rules_get": {
      const { id } = args as { id: string };
      return getJson<T>(`${base}/rules/${enc(id)}`);
    }
    case "rules_list":
      return getJson<T>(`${base}/rules`);
    case "rules_delete": {
      const { id } = args as { id: string };
      return delJson<T>(`${base}/rules/${enc(id)}`);
    }

    default:
      throw new Error(`unknown command: ${cmd}`);
  }
}

/** DELETE a route. Returns undefined for `204`, else the JSON body (e.g. the revoked/removed count). */
async function delJson<T>(url: string): Promise<T> {
  const res = await fetch(url, { method: "DELETE", headers: authHeaders() });
  if (!res.ok) throw new Error(await errorText(res));
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

/** PATCH a JSON body. Returns the JSON response (the stored item). */
async function patchJson<T>(url: string, body: unknown): Promise<T> {
  const res = await fetch(url, {
    method: "PATCH",
    headers: { "content-type": "application/json", ...authHeaders() },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(await errorText(res));
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

async function getJson<T>(url: string): Promise<T> {
  const res = await fetch(url, { headers: authHeaders() });
  if (!res.ok) throw new Error(await errorText(res));
  return (await res.json()) as T;
}

/** POST a JSON body. Some routes return `204 No Content` (resolve/add) — those resolve to undefined. */
async function postJson<T>(url: string, body: unknown, auth = true): Promise<T> {
  const res = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json", ...(auth ? authHeaders() : {}) },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(await errorText(res));
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

/** A 401 is an unauthenticated session; a 403 is the host's capability `Denied`. Surface the body. */
async function errorText(res: Response): Promise<string> {
  const body = await res.text().catch(() => "");
  return body || `request failed (${res.status})`;
}
