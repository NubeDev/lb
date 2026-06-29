// The datasources hook — the one place the admin page assembles the roster + CRUD + probe state from
// the real `datasource.*` verb clients (rules-workbench scope, Phase 3). No fake/demo data — every call
// goes through the real `invoke` seam to the gateway/host. One hook per file (FILE-LAYOUT).

import { useCallback, useEffect, useState } from "react";

import {
  addDatasource,
  listDatasources,
  removeDatasource,
  testDatasource,
  type AddDatasource,
  type DatasourceSummary,
  type ProbeResult,
} from "@/lib/datasources";

export interface Datasources {
  sources: DatasourceSummary[];
  error: string | null;
  /** Per-source probe results, keyed by name (green/red). */
  probes: Record<string, ProbeResult>;
  refresh: () => Promise<void>;
  add: (input: AddDatasource) => Promise<void>;
  remove: (name: string) => Promise<void>;
  probe: (name: string) => Promise<void>;
}

export function useDatasources(): Datasources {
  const [sources, setSources] = useState<DatasourceSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [probes, setProbes] = useState<Record<string, ProbeResult>>({});

  const refresh = useCallback(async () => {
    try {
      setSources(await listDatasources());
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

  const probe = useCallback(async (name: string) => {
    // `testDatasource` never throws — it returns an honest red on a non-200 (never a fabricated green).
    const result = await testDatasource(name);
    setProbes((prev) => ({ ...prev, [name]: result }));
  }, []);

  return {
    sources,
    error,
    probes,
    refresh,
    add: (input) => run(() => addDatasource(input)),
    remove: (name) => run(() => removeDatasource(name)),
    probe,
  };
}
