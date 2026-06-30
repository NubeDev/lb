// The resolved-effective-caps hook (access-console scope) — fetches a subject's resolved caps WITH
// provenance via `authz.resolve` (the sourced twin of the session-mint fold). One hook per file
// (FILE-LAYOUT). Data only — the provenance UI is `EffectiveCaps`.

import { useCallback, useEffect, useState } from "react";

import { resolveCaps, type SourcedCap } from "@/lib/admin/grants.api";

export interface ResolveCapsState {
  caps: SourcedCap[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

/** Resolve `subject`'s effective caps with provenance. `null` subject → empty, no fetch. */
export function useResolveCaps(subject: string | null): ResolveCapsState {
  const [caps, setCaps] = useState<SourcedCap[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!subject) {
      setCaps([]);
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      setCaps(await resolveCaps(subject));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [subject]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { caps, loading, error, refresh };
}
