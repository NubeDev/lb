// The workspaces hook — data + state for the workspace switcher (collaboration scope, slice 2).
// Lists the node directory and creates new entries. One hook per file (FILE-LAYOUT). The *current*
// workspace lives in the session (you re-login to switch the hard wall), so this hook is the
// directory list + create, not the selection.

import { useCallback, useEffect, useState } from "react";

import { createWorkspace, listWorkspaces } from "@/lib/workspace/workspace.api";
import type { WorkspaceRecord } from "@/lib/workspace/workspace.types";

export interface WorkspacesState {
  workspaces: WorkspaceRecord[];
  error: string | null;
  refresh: () => Promise<void>;
  create: (ws: string, name: string) => Promise<void>;
}

/** Drive the workspace directory list + create. Reloads after a create so the switcher updates. */
export function useWorkspaces(): WorkspacesState {
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

  const create = useCallback(
    async (ws: string, name: string) => {
      try {
        await createWorkspace(ws, name);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [refresh],
  );

  return { workspaces, error, refresh, create };
}
