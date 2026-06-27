// The members hook — data + state for the members/teams view (collaboration scope, slice 3). Lists a
// team's members and adds one. Minimal by design (the scope's lean: list + add; full team CRUD is a
// follow-up). One hook per file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import { addMember, listMembers } from "@/lib/members/members.api";

export interface MembersState {
  members: string[];
  error: string | null;
  refresh: () => Promise<void>;
  add: (user: string) => Promise<void>;
}

/** Drive the member list + add for `team` (within the session workspace). Reloads after an add. */
export function useMembers(team: string): MembersState {
  const [members, setMembers] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setMembers(await listMembers(team));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [team]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const add = useCallback(
    async (user: string) => {
      try {
        await addMember(team, user);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [team, refresh],
  );

  return { members, error, refresh, add };
}
