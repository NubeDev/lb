// The switcher's data: the node's workspace directory (`workspace_list`) merged with the
// workspaces we hold stored sessions for. Switching re-activates a stored token or re-mints by
// re-login (the sdk's switchWorkspace); the active workspace is ALWAYS derived from the signed
// token, never client state.

import { useCallback, useEffect, useState } from 'react';
import { gatewayClient } from '../../lib/client';

export interface WorkspaceEntry {
  ws: string;
  /** True when a session token for it is already stored on this device. */
  stored: boolean;
}

export function useWorkspaces(activeWs: string | undefined): {
  workspaces: WorkspaceEntry[];
  switchTo: (ws: string) => Promise<void>;
  error: string;
} {
  const [workspaces, setWorkspaces] = useState<WorkspaceEntry[]>([]);
  const [error, setError] = useState('');

  useEffect(() => {
    const client = gatewayClient();
    if (!client || !activeWs) return;
    let live = true;
    client
      .invoke<{ ws: string }[]>('workspace_list')
      .then((rows) => {
        if (!live) return;
        const stored = new Set(client.session.workspaces());
        setWorkspaces(rows.map((r) => ({ ws: r.ws, stored: stored.has(r.ws) })));
      })
      .catch((e: unknown) => live && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
    };
  }, [activeWs]);

  const switchTo = useCallback(async (ws: string) => {
    setError('');
    try {
      await gatewayClient()?.switchWorkspace(ws);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  return { workspaces, switchTo, error };
}
