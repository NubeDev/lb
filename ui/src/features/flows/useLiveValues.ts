// The live-values hook (flow-deploy-ux scope) — owns the OBSERVE cost so FlowCanvas owns rendering
// (FILE-LAYOUT). It holds the persistent node-state (`flow_node_state` last-values, Decision 5) and
// the live-values on/off toggle. It fetches node-state on open when live values are on; the re-poll
// interval (keyed off the flow being RUNNING) lives in the canvas. Default ON — a flow is a live
// runtime, so opening it should show current values. OFF stops all fetching (the canvas paints the
// last snapshot statically).

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

/** Owns the persistent node-state + the live-values toggle. The re-poll that keeps a RUNNING flow's
 *  values fresh lives in the canvas (it needs the derived `enabled`); this hook owns the state and the
 *  loader it calls. Fetches on open when on (default on); OFF fetches nothing. */
export function useLiveValues(flowId: string, refreshRuns: (flowId: string) => Promise<void>): LiveValues {
  const [nodeState, setNodeState] = useState<FlowNodeState | null>(null);
  // Default ON: a flow is a live runtime, so opening it should show its current values without the
  // operator hunting for a toggle (the PLC expectation). Turning it OFF stops the observe cost.
  const [liveValues, setLiveValues] = useState(true);

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
