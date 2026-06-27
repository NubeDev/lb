// The in-memory members fake (TEST-ONLY) — mirrors the gateway `members_list` / `members_add`.
// Workspace-scoped (the workspace comes from the session store, as the real gateway derives it from
// the token): a team's members in ws-A are invisible in ws-B. Returns `null` for unowned commands.

import { getSession } from "@/lib/session/session.store";

const teams = new Map<string, Map<string, Set<string>>>(); // ws → (team → members)

function ws(): string {
  return getSession()?.workspace ?? "";
}

export function membersFakeInvoke<T>(cmd: string, args?: Record<string, unknown>): T | null {
  switch (cmd) {
    case "members_list": {
      const { team } = args as { team: string };
      const byTeam = teams.get(ws());
      return [...(byTeam?.get(team) ?? new Set<string>())] as T;
    }
    case "members_add": {
      const { team, user } = args as { team: string; user: string };
      const byTeam = teams.get(ws()) ?? new Map<string, Set<string>>();
      const members = byTeam.get(team) ?? new Set<string>();
      members.add(user);
      byTeam.set(team, members);
      teams.set(ws(), byTeam);
      return undefined as T;
    }
    default:
      return null;
  }
}

/** Test helper: clear the fake teams between tests. */
export function __resetMembersFake(): void {
  teams.clear();
}
