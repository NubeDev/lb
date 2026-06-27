// The members-admin hook — list/add/remove a team's members (admin-console scope). Extends the
// collaboration members hook with the missing destructive `remove`. Refetches after each mutation.
// One hook per file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import { addMember, listMembers, removeMember } from "@/lib/members/members.api";

export interface MembersAdminState {
  members: string[];
  error: string | null;
  refresh: () => Promise<void>;
  add: (user: string) => Promise<void>;
  remove: (user: string) => Promise<void>;
}

export function useMembersAdmin(team: string): MembersAdminState {
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
    members,
    error,
    refresh,
    add: (user) => run(() => addMember(team, user)),
    remove: (user) => run(() => removeMember(team, user)),
  };
}
