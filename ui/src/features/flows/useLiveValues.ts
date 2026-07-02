// The live-values hook (flow-deploy-ux scope) — owns the OBSERVE cost so FlowCanvas owns rendering
// (FILE-LAYOUT). It holds the persistent node-state (`flow_node_state` last-values, Decision 5), the
// live-values on/off toggle, and the two polling effects that keep an armed flow's values fresh. When
// live values are OFF nothing is fetched and no interval runs — the canvas paints the last snapshot
// statically (Node-RED debug is opt-in). When ON, it fetches once immediately and re-polls on a slow
// tick while the flow is armed so each firing's values surface without reopening.
//
// `armedKind` is passed IN (the canvas derives it from the nodeState this hook returns — no
// circularity; the effect just reads the latest value each render). Default OFF.

import { useCallback, useEffect, useState } from "react";

import { getFlowNodeState, type FlowNodeState } from "@/lib/flows";

export interface LiveValues {
  /** The persistent per-node last-value view, or null before first load / when off + never loaded. */
  nodeState: FlowNodeState | null;
  /** Whether live-value painting is on. */
  liveValues: boolean;
  /** Fetch the node-state now (the initial open, the armed re-poll, and the enable toggle all use it). */
  loadNodeState: (flowId: string) => Promise<void>;
  /** Turn live values on/off. Turning ON fetches immediately; OFF stops future fetches. */
  toggleLiveValues: (next: boolean) => void;
}

/** Owns the persistent node-state + the live-values toggle. The polling that keeps an armed flow's
 *  values fresh lives in the canvas (it needs the derived `armed.kind`); this hook owns the state and
 *  the loader it calls. Fetches once on open when on; OFF fetches nothing (opt-in observe cost). */
export function useLiveValues(flowId: string, refreshRuns: (flowId: string) => Promise<void>): LiveValues {
  const [nodeState, setNodeState] = useState<FlowNodeState | null>(null);
  const [liveValues, setLiveValues] = useState(false);

  const loadNodeState = useCallback(async (id: string) => {
    try {
      setNodeState(await getFlowNodeState(id));
    } catch {
      /* a fresh flow with no state yet — leave null, nodes render blank */
    }
  }, []);

  // Fetch once on open (and on flow switch) when live values are on. Off → no fetch (opt-in cost).
  useEffect(() => {
    if (!liveValues) return;
    void loadNodeState(flowId);
  }, [flowId, loadNodeState, liveValues]);

  const toggleLiveValues = useCallback(
    (next: boolean) => {
      setLiveValues(next);
      if (next) {
        void loadNodeState(flowId);
        void refreshRuns(flowId);
      }
    },
    [flowId, loadNodeState, refreshRuns],
  );

  return { nodeState, liveValues, loadNodeState, toggleLiveValues };
}
