// The roles hook — list the workspace's roles WITH the caps each bundles, and define new ones
// (admin-console redesign). This is the real role editor's data: `roles.list` already returns the
// caps (the old UI threw them away), and `roles.define` is no-widening server-side. One hook per
// file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import { defineRole, deleteRole, listRoles, type RoleView } from "@/lib/admin/roles.api";

export interface RolesState {
  roles: RoleView[];
  error: string | null;
  refresh: () => Promise<void>;
  define: (name: string, caps: string[]) => Promise<void>;
  /** Delete role `name` (cascade-un-assigns it). Resolves to the affected-subject count. */
  remove: (name: string) => Promise<number>;
}

export function useRoles(): RolesState {
  const [roles, setRoles] = useState<RoleView[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setRoles(await listRoles());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return {
    roles,
    error,
    refresh,
    define: async (name, caps) => {
      try {
        await defineRole(name, caps);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    remove: async (name) => {
      try {
        const { affected } = await deleteRole(name);
        await refresh();
        return affected;
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        return 0;
      }
    },
  };
}
