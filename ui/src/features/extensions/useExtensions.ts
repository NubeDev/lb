// The extensions-console hook — list installed extensions (both tiers, live state) + lifecycle
// (enable/disable = start/stop, uninstall) (admin-console + lifecycle-management scopes). Refetches
// after each mutation so the table reflects the new state. One hook per file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import {
  disableExtension,
  enableExtension,
  listExtensions,
  publishArtifact,
  resetExtension,
  uninstallExtension,
  type Artifact,
  type ExtRow,
} from "@/lib/ext/ext.api";
export interface ExtensionsState {
  rows: ExtRow[];
  error: string | null;
  refresh: () => Promise<void>;
  setEnabled: (ext: string, enabled: boolean) => Promise<void>;
  reset: (ext: string) => Promise<void>;
  uninstall: (ext: string) => Promise<void>;
  upload: (artifact: Artifact) => Promise<void>;
}

export function useExtensions(): ExtensionsState {
  const [rows, setRows] = useState<ExtRow[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      // Always reads the real node — an empty list means the workspace has no extensions installed
      // (honest), never a fabricated demo set (the fake seed is gone).
      setRows(await listExtensions());
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
    rows,
    error,
    refresh,
    setEnabled: (ext, enabled) => run(() => (enabled ? enableExtension(ext) : disableExtension(ext))),
    reset: (ext) => run(() => resetExtension(ext)),
    uninstall: (ext) => run(() => uninstallExtension(ext)),
    upload: (artifact) => run(() => publishArtifact(artifact)),
  };
}
