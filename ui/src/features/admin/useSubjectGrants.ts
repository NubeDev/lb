// The subject-grants hook — read + mutate the access of any `kind:name` subject (admin-console
// redesign). Used for both a user (`user:bob`) and a team (`team:eng`), so People and Teams share
// one access editor. Splits the flat grant list the host returns into ROLES (the `role:<name>`
// synthetic caps) and direct CAPS (everything else), because the UI shows them as two distinct
// things: a role dropdown vs an advanced raw-cap list. Assigning a role IS a grant of `role:<name>`.
// One hook per file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import { assignGrant, listGrants, revokeGrant } from "@/lib/admin/grants.api";

const ROLE_PREFIX = "role:";

export interface SubjectGrants {
  /** Role names assigned to the subject (the `role:` prefix stripped). */
  roles: string[];
  /** Direct capability strings (non-role grants). */
  caps: string[];
  error: string | null;
  refresh: () => Promise<void>;
  assignRole: (role: string) => Promise<void>;
  revokeRole: (role: string) => Promise<void>;
  assignCap: (cap: string) => Promise<void>;
  revokeCap: (cap: string) => Promise<void>;
}

export function useSubjectGrants(subject: string | null): SubjectGrants {
  const [roles, setRoles] = useState<string[]>([]);
  const [caps, setCaps] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!subject) {
      setRoles([]);
      setCaps([]);
      return;
    }
    try {
      const all = await listGrants(subject);
      setRoles(all.filter((c) => c.startsWith(ROLE_PREFIX)).map((c) => c.slice(ROLE_PREFIX.length)));
      setCaps(all.filter((c) => !c.startsWith(ROLE_PREFIX)));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [subject]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = useCallback(
    async (op: () => Promise<unknown>) => {
      if (!subject) return;
      try {
        await op();
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [subject, refresh],
  );

  return {
    roles,
    caps,
    error,
    refresh,
    assignRole: (role) => run(() => assignGrant(subject!, `${ROLE_PREFIX}${role}`)),
    revokeRole: (role) => run(() => revokeGrant(subject!, `${ROLE_PREFIX}${role}`)),
    assignCap: (cap) => run(() => assignGrant(subject!, cap)),
    revokeCap: (cap) => run(() => revokeGrant(subject!, cap)),
  };
}
