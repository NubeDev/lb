// The System-page hook — data + state for the admin system map (system-map scope). Loads the status
// grid (`system.overview`) eagerly and the topology (`system.topology`) lazily (when the user flips to
// the graph), and exposes a single `refresh` that re-fetches whatever is showing — honest for a
// debugging console you open deliberately (poll-on-open, no live feed in v1). READ-ONLY: no mutation
// here (the map is a read; control verbs live in their own scopes). One hook per file (FILE-LAYOUT).
// Everything runs against the real gateway, admin-gated server-side.

import { useCallback, useEffect, useState } from "react";

import { systemOverview, systemTopology } from "@/lib/system/system.api";
import type { SystemOverview, SystemTopology } from "@/lib/system/system.types";

export interface SystemState {
  overview: SystemOverview | null;
  topology: SystemTopology | null;
  error: string | null;
  loading: boolean;
  /** Re-fetch the overview (and the topology if it has already been loaded). */
  refresh: () => Promise<void>;
  /** Lazily load the topology the first time the graph view is opened. */
  loadTopology: () => Promise<void>;
}

/** Drive the System page for the session workspace. */
export function useSystem(): SystemState {
  const [overview, setOverview] = useState<SystemOverview | null>(null);
  const [topology, setTopology] = useState<SystemTopology | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const fail = (e: unknown) => setError(e instanceof Error ? e.message : String(e));

  const loadTopology = useCallback(async () => {
    try {
      const topo = await systemTopology();
      setTopology(topo);
      setError(null);
    } catch (e) {
      fail(e);
    }
  }, []);

  // Re-read the overview, plus the topology if the graph has already been opened — both project from
  // one server snapshot, so refreshing both keeps the grid and graph from disagreeing.
  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const ov = await systemOverview();
      setOverview(ov);
      setError(null);
      if (topology) await loadTopology();
    } catch (e) {
      fail(e);
    } finally {
      setLoading(false);
    }
  }, [topology, loadTopology]);

  useEffect(() => {
    void systemOverview()
      .then((ov) => {
        setOverview(ov);
        setError(null);
      })
      .catch(fail);
  }, []);

  return { overview, topology, error, loading, refresh, loadTopology };
}
