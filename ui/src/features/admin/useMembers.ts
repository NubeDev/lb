// The members hook — the workspace roster for the Access console People tab (global-identity scope,
// decision #9). Lists the effective members (membership ∪ legacy users) and supports add/remove.
// One hook per file (FILE-LAYOUT). Reads through the real `invoke` seam to the gateway/host.

import { useCallback, useEffect, useState } from "react";

import { addMember, listMembers, removeMember, type MembershipView } from "@/lib/membership/membership.api";

export interface MembersState {
  members: MembershipView[];
  error: string | null;
  refresh: () => Promise<void>;
  add: (sub: string) => Promise<void>;
  remove: (sub: string) => Promise<void>;
}

export function useMembers(): MembersState {
  const [members, setMembers] = useState<MembershipView[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setMembers(await listMembers());
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
    members,
    error,
    refresh,
    add: (sub) => run(() => addMember(sub)),
    remove: (sub) => run(() => removeMember(sub)),
  };
}
