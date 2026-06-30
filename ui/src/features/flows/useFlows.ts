// The flows CRUD state hook (flows-canvas scope, Wave 3). Holds the workspace roster + the
// currently-open flow + the palette registry, and the load/save/delete actions over the real api
// client. Separated from the view markup (FILE-LAYOUT frontend: data in the hook, markup in the .tsx).
// The palette re-fetches on focus so a swapped node-providing extension re-populates without a
// restart (the hot-reload claim, at the UI boundary).

import { useCallback, useEffect, useState } from "react";

import {
  deleteFlow,
  getFlow,
  listFlows,
  listFlowNodes,
  saveFlow,
  type Flow,
  type FlowSummary,
  type NodeDescriptor,
} from "@/lib/flows";

export interface FlowsState {
  roster: FlowSummary[];
  open: Flow | null;
  palette: NodeDescriptor[];
  error: string | null;
  refresh: () => Promise<void>;
  load: (id: string) => Promise<void>;
  save: (flow: Flow) => Promise<{ ok: boolean; version?: number; error?: string }>;
  remove: (id: string) => Promise<void>;
  setOpen: (flow: Flow | null) => void;
}

export function useFlows(ws: string): FlowsState {
  const [roster, setRoster] = useState<FlowSummary[]>([]);
  const [open, setOpen] = useState<Flow | null>(null);
  const [palette, setPalette] = useState<NodeDescriptor[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [flows, nodes] = await Promise.all([listFlows(), listFlowNodes()]);
      setRoster(flows);
      setPalette(nodes);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  // Re-load the roster + palette whenever the workspace changes (a fresh session = a fresh roster,
  // the wall; a fresh workspace's installed extensions = a fresh node set).
  useEffect(() => {
    void refresh();
    // Hot-reload: refetch the palette on window focus so a newly-installed node-providing extension
    // re-populates the menu without a canvas restart.
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [ws, refresh]);

  const load = useCallback(async (id: string) => {
    setError(null);
    try {
      setOpen(await getFlow(id));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  /** Save a flow; surfaces the host's validation message inline (no throw to the canvas) so an
   *  invalid DAG or schema-invalid node config renders its `400` text rather than crashing. Returns
   *  the new version the host allocated (Decision 1). */
  const save = useCallback(
    async (flow: Flow): Promise<{ ok: boolean; version?: number; error?: string }> => {
      try {
        const { version } = await saveFlow(flow);
        setOpen({ ...flow, version });
        await refresh();
        return { ok: true, version };
      } catch (e) {
        return { ok: false, error: e instanceof Error ? e.message : String(e) };
      }
    },
    [refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      await deleteFlow(id);
      setOpen((cur) => (cur?.id === id ? null : cur));
      await refresh();
    },
    [refresh],
  );

  return { roster, open, palette, error, refresh, load, save, remove, setOpen };
}
