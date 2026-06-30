// The "my workspaces" hook — the workspaces the current identity is a member of, resolved through
// `identity.workspaces` (global-identity scope). Drives the workspace switcher so it shows the
// identity's actual roster, not just the node directory. Falls back to the directory on error/empty.

import { useCallback, useEffect, useState } from "react";

import { identityWorkspaces, type IdentityWorkspace } from "@/lib/identity/identity.api";

export interface MyWorkspacesState {
  mine: IdentityWorkspace[];
  error: string | null;
  refresh: () => Promise<void>;
}

/** Resolve the workspaces `sub` belongs to. Best-effort: an error leaves `mine` empty (the switcher
 *  falls back to the node directory). Refreshed on mount + when the sub changes. */
export function useMyWorkspaces(sub: string | undefined): MyWorkspacesState {
  const [mine, setMine] = useState<IdentityWorkspace[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!sub) return;
    try {
      setMine(await identityWorkspaces(sub));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [sub]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { mine, error, refresh };
}
