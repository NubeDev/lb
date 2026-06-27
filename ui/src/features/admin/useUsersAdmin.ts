// The users-admin hook — data + mutations for UsersAdmin (admin-console scope). Lists users, creates,
// disables/enables, deletes (returning the revoked-grant count for the consequence). Refetches after
// each mutation (the scope's lean: refetch-after-mutation). One hook per file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import {
  createUser,
  deleteUser,
  disableUser,
  enableUser,
  listUsers,
  type UserView,
} from "@/lib/admin/users.api";

export interface UsersAdminState {
  users: UserView[];
  error: string | null;
  refresh: () => Promise<void>;
  create: (user: string) => Promise<void>;
  setActive: (user: string, active: boolean) => Promise<void>;
  remove: (user: string) => Promise<void>;
}

export function useUsersAdmin(): UsersAdminState {
  const [users, setUsers] = useState<UserView[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setUsers(await listUsers());
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
    error,
    refresh,
    create: (user) => run(() => createUser(user)),
    setActive: (user, active) => run(() => (active ? enableUser(user) : disableUser(user))),
    remove: (user) => run(() => deleteUser(user)),
  };
}
