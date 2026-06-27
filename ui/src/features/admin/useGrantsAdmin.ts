// The grants-admin hook — read a subject's grants + assign/revoke (admin-console scope: NO role
// editor this slice). Lists the caps granted to a `kind:name` subject and the workspace's role names;
// assign/revoke a cap (no-widening is enforced server-side). Refetches after each mutation. One hook
// per file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import { assignGrant, listGrants, listRoles, revokeGrant } from "@/lib/admin/grants.api";

export interface GrantsAdminState {
  grants: string[];
  roles: string[];
  error: string | null;
  refresh: () => Promise<void>;
  assign: (cap: string) => Promise<void>;
  revoke: (cap: string) => Promise<void>;
}

export function useGrantsAdmin(subject: string): GrantsAdminState {
  const [grants, setGrants] = useState<string[]>([]);
  const [roles, setRoles] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [g, r] = await Promise.all([listGrants(subject), listRoles()]);
      setGrants(g);
      setRoles(r);
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
    grants,
    roles,
    error,
    refresh,
    assign: (cap) => run(() => assignGrant(subject, cap)),
    revoke: (cap) => run(() => revokeGrant(subject, cap)),
  };
}
