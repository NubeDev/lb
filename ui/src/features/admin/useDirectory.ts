// The directory hook — the one place the admin console assembles "who belongs to who" from the real
// endpoints (admin-console redesign). Loads the workspace's users, team records, and every team's
// membership, then inverts memberships into `teamsByUser` so the People and Teams tabs can show the
// relationship without making the operator type ids. User CRUD lives here too (create/active/delete)
// so the People table mutates and refetches in one place. No fake/demo data — every call goes through
// the real `invoke` seam to the gateway/host. One hook per file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import {
  createUser,
  deleteUser,
  disableUser,
  enableUser,
  listUsers,
  type UserView,
} from "@/lib/admin/users.api";
import {
  createTeam,
  deleteTeam,
  listTeams,
  renameTeam,
  type TeamView,
} from "@/lib/admin/teams.api";
import { addMember, listMembers, removeMember } from "@/lib/members/members.api";

export interface Directory {
  users: UserView[];
  teams: TeamView[];
  /** team id → the `user:…` ids in it. */
  membersByTeam: Record<string, string[]>;
  /** bare user id → the team ids they belong to. */
  teamsByUser: Record<string, string[]>;
  error: string | null;
  refresh: () => Promise<void>;
  create: (user: string) => Promise<void>;
  setActive: (user: string, active: boolean) => Promise<void>;
  remove: (user: string) => Promise<void>;
  createTeamRecord: (team: string) => Promise<void>;
  renameTeamRecord: (team: string, name: string) => Promise<void>;
  removeTeamRecord: (team: string) => Promise<void>;
  addTeamMember: (team: string, user: string) => Promise<void>;
  removeTeamMember: (team: string, user: string) => Promise<void>;
}

/** Strip the `user:` prefix the members api returns, leaving the bare id used elsewhere. */
function bare(id: string): string {
  return id.startsWith("user:") ? id.slice("user:".length) : id;
}

export function useDirectory(): Directory {
  const [users, setUsers] = useState<UserView[]>([]);
  const [teams, setTeams] = useState<TeamView[]>([]);
  const [membersByTeam, setMembersByTeam] = useState<Record<string, string[]>>({});
  const [teamsByUser, setTeamsByUser] = useState<Record<string, string[]>>({});
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [u, t] = await Promise.all([listUsers(), listTeams()]);
      const byTeam: Record<string, string[]> = {};
      const byUser: Record<string, string[]> = {};
      await Promise.all(
        t.map(async (team) => {
          const members = await listMembers(team.team);
          byTeam[team.team] = members;
          for (const m of members) {
            const id = bare(m);
            (byUser[id] ??= []).push(team.team);
          }
        }),
      );
      setUsers(u);
      setTeams(t);
      setMembersByTeam(byTeam);
      setTeamsByUser(byUser);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = useCallback(
    async (op: () => Promise<unknown>) => {
      try {
        await op();
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [refresh],
  );

  return {
    users,
    teams,
    membersByTeam,
    teamsByUser,
    error,
    refresh,
    create: (user) => run(() => createUser(user)),
    setActive: (user, active) => run(() => (active ? enableUser(user) : disableUser(user))),
    remove: (user) => run(() => deleteUser(user)),
    createTeamRecord: (team) => run(() => createTeam(team, team)),
    renameTeamRecord: (team, name) => run(() => renameTeam(team, name)),
    removeTeamRecord: (team) => run(() => deleteTeam(team)),
    addTeamMember: (team, user) => run(() => addMember(team, user)),
    removeTeamMember: (team, user) => run(() => removeMember(team, user)),
  };
}
