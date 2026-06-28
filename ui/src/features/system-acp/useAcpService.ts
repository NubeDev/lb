// The ACP service page hook — data + state for the ACP adapter detail (tool-catalog scope). Loads the
// adapter's static protocol/capability facts (`system.acp`) on open and exposes a `refresh`. ACP is a
// per-stdio-session adapter (not a polled server), so there is no live health here — these are
// reachable capability facts, read once on open. READ-ONLY. One hook per file (FILE-LAYOUT). Runs
// against the real gateway, admin-gated server-side.

import { useCallback, useEffect, useState } from "react";

import { systemAcp } from "@/lib/system/system.api";
import type { AcpInfo } from "@/lib/system/system.types";

export interface AcpServiceState {
  info: AcpInfo | null;
  error: string | null;
  loading: boolean;
  refresh: () => Promise<void>;
}

/** Drive the ACP service page for the session workspace. */
export function useAcpService(): AcpServiceState {
  const [info, setInfo] = useState<AcpInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setInfo(await systemAcp());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return { info, error, loading, refresh: load };
}
