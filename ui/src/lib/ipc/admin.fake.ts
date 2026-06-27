// The in-memory admin fake (TEST-ONLY) — mirrors the gateway admin-crud routes 1:1 (admin-crud +
// admin-console scopes): users, team records, workspace lifecycle, grants/roles. Workspace-scoped
// (the ws comes from the session store, as the real gateway derives it from the token). Returns
// `null` for any command it doesn't own so the other fakes still run.
//
// Faithful to the host semantics the views rely on: `user.list` never carries a credential;
// disable/enable flip `active`; delete tombstones (gone from the list); teams.delete cascades the
// member edges + reports the removed count; workspace archive hides + purge needs the typed confirm.

import { getSession } from "@/lib/session/session.store";
import { __removeAllMembers, __teamMembers } from "./members.fake";

export interface UserView {
  user: string;
  active: boolean;
  role: string;
}
interface TeamView {
  team: string;
  name: string;
}

interface RoleView {
  name: string;
  caps: string[];
}

const users = new Map<string, Map<string, UserView>>(); // ws → (user → view)
const teamRecords = new Map<string, Map<string, TeamView>>(); // ws → (team → record)
const grants = new Map<string, Map<string, Set<string>>>(); // ws → (subject → caps)
const roles = new Map<string, Map<string, RoleView>>(); // ws → (role name → role)
const archived = new Set<string>(); // `${ws}/${workspaceId}` soft-deleted
const purged = new Set<string>(); // `${ws}/${workspaceId}` hard-deleted

function ws(): string {
  return getSession()?.workspace ?? "";
}
function map<V>(m: Map<string, Map<string, V>>): Map<string, V> {
  const inner = m.get(ws()) ?? new Map<string, V>();
  m.set(ws(), inner);
  return inner;
}

export function adminFakeInvoke<T>(cmd: string, args?: Record<string, unknown>): T | null {
  switch (cmd) {
    // ── users ──
    case "user_list":
      return [...map(users).values()].sort((a, b) => a.user.localeCompare(b.user)) as T;
    case "user_create": {
      const { user, role } = args as { user: string; role?: string };
      map(users).set(user, { user, active: true, role: role ?? "member" });
      return undefined as T;
    }
    case "user_disable":
    case "user_enable": {
      const { user } = args as { user: string };
      const v = map(users).get(user);
      if (v) v.active = cmd === "user_enable";
      return undefined as T;
    }
    case "user_delete": {
      const { user } = args as { user: string };
      map(users).delete(user);
      const caps = map(grants).get(`user:${user}`);
      const revoked = caps?.size ?? 0;
      map(grants).delete(`user:${user}`);
      return revoked as T;
    }

    // ── team records (membership edges live in the members fake) ──
    case "teams_list":
      return [...map(teamRecords).values()] as T;
    case "teams_create":
    case "teams_rename": {
      const { team, name } = args as { team: string; name: string };
      map(teamRecords).set(team, { team, name });
      return undefined as T;
    }
    case "teams_delete": {
      const { team } = args as { team: string };
      const removed = __teamMembers(ws(), team).length;
      __removeAllMembers(ws(), team);
      map(grants).delete(`team:${team}`);
      map(teamRecords).delete(team);
      return removed as T;
    }

    // ── workspace lifecycle ──
    case "workspace_rename": {
      const { ws: w } = args as { ws: string; name: string };
      archived.delete(`${ws()}/${w}`); // rename un-archives (no resurrection of a purge)
      return undefined as T;
    }
    case "workspace_archive": {
      const { ws: w } = args as { ws: string };
      archived.add(`${ws()}/${w}`);
      return undefined as T;
    }
    case "workspace_purge": {
      const { ws: w, confirm } = args as { ws: string; confirm: string };
      if (confirm !== w) throw new Error("denied"); // typed confirm must match
      purged.add(`${ws()}/${w}`);
      return undefined as T;
    }

    // ── grants / roles (read + assign/revoke) ──
    case "grants_list": {
      const { subject } = args as { subject: string };
      return [...(map(grants).get(subject) ?? new Set<string>())] as T;
    }
    case "grants_assign": {
      const { subject, cap } = args as { subject: string; cap: string };
      const caps = map(grants).get(subject) ?? new Set<string>();
      caps.add(cap);
      map(grants).set(subject, caps);
      return undefined as T;
    }
    case "grants_revoke": {
      const { subject, cap } = args as { subject: string; cap: string };
      map(grants).get(subject)?.delete(cap);
      return undefined as T;
    }
    case "roles_list":
      return [...map(roles).values()].sort((a, b) => a.name.localeCompare(b.name)) as T;
    case "roles_define": {
      const { name, caps } = args as { name: string; caps: string[] };
      map(roles).set(name, { name, caps: [...caps] });
      return undefined as T;
    }

    default:
      return null;
  }
}

/** Test helper: is a workspace soft-archived / hard-purged in the current session ws? */
export function __workspaceState(workspaceId: string): "active" | "archived" | "purged" {
  const k = `${ws()}/${workspaceId}`;
  if (purged.has(k)) return "purged";
  if (archived.has(k)) return "archived";
  return "active";
}

/** Test helper: clear all admin fake state between tests. */
export function __resetAdminFake(): void {
  users.clear();
  teamRecords.clear();
  grants.clear();
  roles.clear();
  archived.clear();
  purged.clear();
}
