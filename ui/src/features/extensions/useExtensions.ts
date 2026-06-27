// The extensions-console hook — list installed extensions (both tiers, live state) + lifecycle
// (enable/disable = start/stop, uninstall) (admin-console + lifecycle-management scopes). Refetches
// after each mutation so the table reflects the new state. One hook per file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import {
  disableExtension,
  enableExtension,
  listExtensions,
  publishArtifact,
  uninstallExtension,
  type Artifact,
  type ExtRow,
} from "@/lib/ext/ext.api";
import { gatewayUrl } from "@/lib/ipc/http";
import { seedDevExtensions } from "@/lib/ipc/ext.fake";

export interface ExtensionsState {
  rows: ExtRow[];
  error: string | null;
  refresh: () => Promise<void>;
  setEnabled: (ext: string, enabled: boolean) => Promise<void>;
  uninstall: (ext: string) => Promise<void>;
  upload: (artifact: Artifact) => Promise<void>;
}

export function useExtensions(): ExtensionsState {
  const [rows, setRows] = useState<ExtRow[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      // DEV-only: in the no-gateway browser/demo build, seed the reference extensions so the console
      // isn't empty. The gateway path (real node) ignores this — `seedDevExtensions` only touches the
      // in-memory fake, and the gateway transport never calls the fake. Skipped under test (where the
      // suites seed explicit fixtures and assert the empty state).
      if (gatewayUrl() === "" && import.meta.env.MODE !== "test") seedDevExtensions();
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
    uninstall: (ext) => run(() => uninstallExtension(ext)),
    upload: (artifact) => run(() => publishArtifact(artifact)),
  };
}
