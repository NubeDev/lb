// The onboarding-wizard data hook (setup scope) — loads everything the Setup wizard needs to onboard
// one person end to end (existing users, teams, roles, navs) and exposes the real write verbs it
// orchestrates: create a user, create+join a team, assign a role to that team, and pick which nav the
// team sees. Every call is a REAL host verb re-checked server-side (no mocks, rule 9); the wizard is
// pure orchestration over the same verbs the People/Teams/Roles/Nav tabs already use. One hook per
// file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import { createUser, listUsers, type UserView } from "@/lib/admin/users.api";
import { createTeam, listTeams, type TeamView } from "@/lib/admin/teams.api";
import { addMember as addTeamMember } from "@/lib/members/members.api";
import { listRoles, type RoleView } from "@/lib/admin/roles.api";
import { assignGrant, resolveCaps, type SourcedCap } from "@/lib/admin/grants.api";
import { listNavs, saveNav, setDefaultNav, shareNav, type NavItem, type NavSummary } from "@/lib/nav";
import { listDashboards, type DashboardSummary } from "@/lib/dashboard";
import { listExtensions, type ExtRow } from "@/lib/ext/ext.api";

export interface SetupSources {
  users: UserView[];
  teams: TeamView[];
  roles: RoleView[];
  navs: NavSummary[];
  /** The real item sources the shared nav composer picks from (dashboards + ext pages). */
  dashboards: DashboardSummary[];
  extensions: ExtRow[];
}

export interface SetupState {
  sources: SetupSources;
  loading: boolean;
  error: string | null;
  reload: () => Promise<void>;
  // ── the real verbs the wizard drives (each reloads the affected source) ──
  makeUser: (user: string) => Promise<void>;
  makeTeam: (team: string, name: string) => Promise<void>;
  joinTeam: (team: string, user: string) => Promise<void>;
  grantRole: (team: string, role: string) => Promise<void>;
  /** Create/replace a nav (the shared composer's output); resolves to its slug id. */
  makeNav: (title: string, items: NavItem[]) => Promise<string>;
  giveNavToTeam: (navId: string, team: string) => Promise<void>;
  makeNavDefault: (navId: string) => Promise<void>;
  /** Resolve a user's EFFECTIVE caps (with provenance) — the honest input to the live preview. */
  effectiveCaps: (user: string) => Promise<SourcedCap[]>;
}

/** Strip a subject prefix (`user:` / `team:`) to the bare id used by the pickers. */
function bare(id: string): string {
  const i = id.indexOf(":");
  return i >= 0 ? id.slice(i + 1) : id;
}

export function useSetup(): SetupState {
  const [sources, setSources] = useState<SetupSources>({
    users: [],
    teams: [],
    roles: [],
    navs: [],
    dashboards: [],
    extensions: [],
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [users, teams, roles, navs, dashboards, extensions] = await Promise.all([
        listUsers().catch(() => [] as UserView[]),
        listTeams().catch(() => [] as TeamView[]),
        listRoles().catch(() => [] as RoleView[]),
        listNavs().catch(() => [] as NavSummary[]),
        listDashboards().catch(() => [] as DashboardSummary[]),
        listExtensions().catch(() => [] as ExtRow[]),
      ]);
      setSources({
        users,
        teams,
        roles,
        navs,
        dashboards,
        // Only ext pages with a UI entry are pickable (mirrors the nav builder's filter).
        extensions: extensions.filter((e) => e.ui?.entry),
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  // Wrap a write so a failure surfaces on the wizard (not swallowed) AND refreshes the sources so the
  // next step sees the new record. Re-throws so the caller's step can stay on the failed action.
  const run = useCallback(
    async (op: () => Promise<unknown>) => {
      try {
        await op();
        await reload();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        throw e;
      }
    },
    [reload],
  );

  return {
    sources,
    loading,
    error,
    reload,
    makeUser: (user) => run(() => createUser(bare(user))),
    makeTeam: (team, name) => run(() => createTeam(bare(team), name)),
    joinTeam: (team, user) => run(() => addTeamMember(bare(team), bare(user))),
    // Assigning a role to a subject is a grant of the synthetic `role:<name>` cap (grants.api). We
    // grant to the TEAM so every current + future member inherits it — the "give the team these
    // pages" shape from the nav scope (a role grants; a nav shapes).
    grantRole: (team, role) => run(() => assignGrant(`team:${bare(team)}`, `role:${role}`)),
    // Create the nav via the same `nav.save` verb the Nav tab uses — the id is a slug of the title
    // (LWW upsert). Returns the id so the wizard can immediately share it to the team.
    makeNav: async (title, items) => {
      const id = title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
      await run(() => saveNav(id, title, items));
      return id;
    },
    // Share the nav to the team (team tier) — the members see it resolved as their menu. Pure lens.
    giveNavToTeam: (navId, team) => run(() => shareNav(navId, "team", `team:${bare(team)}`)),
    makeNavDefault: (navId) => run(() => setDefaultNav(navId)),
    // Read-only: never mutates, so it doesn't go through `run` (no reload needed).
    effectiveCaps: (user) => resolveCaps(`user:${bare(user)}`),
  };
}
