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
//   channel_post     → POST /channels/{cid}/messages
//   channel_history  → GET  /channels/{cid}/messages
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

/** The gateway base URL, e.g. `http://127.0.0.1:8080`. Empty string = same origin. */
export function gatewayUrl(): string {
  return (import.meta.env.VITE_GATEWAY_URL as string | undefined) ?? "";
}

/** The Authorization header for the current session, or none when logged out. */
function authHeaders(): Record<string, string> {
  const token = sessionToken();
  return token ? { authorization: `Bearer ${token}` } : {};
}

const enc = encodeURIComponent;

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
