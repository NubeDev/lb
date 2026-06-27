// The teams-admin hook — data + mutations for TeamsAdmin (admin-console scope). Lists team records,
// creates, renames, deletes (returning the cascade member-removed count). Refetches after each
// mutation. One hook per file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import {
  createTeam,
  deleteTeam,
  listTeams,
  renameTeam,
  type TeamView,
} from "@/lib/admin/teams.api";

export interface TeamsAdminState {
  teams: TeamView[];
  error: string | null;
  refresh: () => Promise<void>;
  create: (team: string, name: string) => Promise<void>;
  rename: (team: string, name: string) => Promise<void>;
  remove: (team: string) => Promise<number>;
}

export function useTeamsAdmin(): TeamsAdminState {
  const [teams, setTeams] = useState<TeamView[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setTeams(await listTeams());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const create = useCallback(
    async (team: string, name: string) => {
      try {
        await createTeam(team, name);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [refresh],
  );

  const rename = useCallback(
    async (team: string, name: string) => {
      try {
        await renameTeam(team, name);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [refresh],
  );

  const remove = useCallback(
    async (team: string) => {
      const removed = await deleteTeam(team);
      await refresh();
      return removed;
    },
    [refresh],
  );

  return { teams, error, refresh, create, rename, remove };
}
