// The workspaces-admin hook — list + lifecycle (rename / archive / purge) (admin-console scope).
// Lists the node directory (collaboration `workspace_list`); archive is the reversible soft-delete,
// purge the hard one (typed confirm == ws id + the server's `workspace.purge` cap). Refetches after
// each mutation. One hook per file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import { createWorkspace, listWorkspaces } from "@/lib/workspace/workspace.api";
import type { WorkspaceRecord } from "@/lib/workspace/workspace.types";
import {
  archiveWorkspace,
  purgeWorkspace,
  renameWorkspace,
} from "@/lib/admin/workspaces.api";

export interface WorkspacesAdminState {
  workspaces: WorkspaceRecord[];
  error: string | null;
  refresh: () => Promise<void>;
  /** Register a new workspace (id + display name) in the directory. Mirrors `workspace.create`. */
  create: (ws: string, name: string) => Promise<void>;
  rename: (ws: string, name: string) => Promise<void>;
  archive: (ws: string) => Promise<void>;
  purge: (ws: string, confirm: string) => Promise<void>;
}

export function useWorkspacesAdmin(): WorkspacesAdminState {
  const [workspaces, setWorkspaces] = useState<WorkspaceRecord[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setWorkspaces(await listWorkspaces());
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
    workspaces,
    error,
    refresh,
    create: (ws, name) => run(() => createWorkspace(ws, name)),
    rename: (ws, name) => run(() => renameWorkspace(ws, name)),
    archive: (ws) => run(() => archiveWorkspace(ws)),
    purge: (ws, confirm) => run(() => purgeWorkspace(ws, confirm)),
  };
}
