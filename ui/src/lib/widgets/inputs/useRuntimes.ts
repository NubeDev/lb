// The runtime-list hook (external-agent run-lifecycle #5) — fetches the node's configured agent
// runtimes ONCE on mount so the `RuntimeArg` dropdown has its options + default. Mirrors `useCatalog`
// (data only, no markup — FILE-LAYOUT). A fetch error surfaces as `error` (the widget degrades to the
// default id); the default id drives the picker's initial selection.

import { useEffect, useState } from "react";

import { agentRuntimes } from "@/lib/agent/runtimes.api";

export interface RuntimesState {
  /** The configured runtime ids (sorted). Empty until the first fetch resolves. */
  runtimes: string[];
  /** The default runtime id (`"default"`) — the picker's initial + fallback selection. */
  defaultId: string;
  loading: boolean;
  error: string | null;
}

/** Load the node's configured agent runtimes for the picker. Runs once on mount. */
export function useRuntimes(): RuntimesState {
  const [runtimes, setRuntimes] = useState<string[]>([]);
  const [defaultId, setDefaultId] = useState("default");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const r = await agentRuntimes();
        if (!live) return;
        setRuntimes(r.runtimes);
        setDefaultId(r.default);
        setError(null);
      } catch (e) {
        if (!live) return;
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        if (live) setLoading(false);
      }
    })();
    return () => {
      live = false;
    };
  }, []);

  return { runtimes, defaultId, loading, error };
}
