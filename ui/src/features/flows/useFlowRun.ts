// The bounded settle-poll hook (flows-canvas scope, Wave 3 — the RESOLVED poll decision, mirroring
// `useChainRun`). Given a run id, poll `flows.runs.get` on a FIXED interval WHILE the status is
// non-terminal and STOP on a terminal status (success/partialFailure/failed). It is NEVER an
// unbounded setInterval: a late open does one snapshot and stops if already terminal. `flows.watch`
// SSE is the named follow-up that replaces this poll later (flows-canvas-scope non-goal).
//
// Also provides `reattach(flowId)` — on open, find the active run via `flows.runs.list
// {status:"active"}` so a canvas reopened mid-run can rejoin it (the explicit step in the scope's
// "Reattach to an active run on open" risk).

import { useCallback, useEffect, useRef, useState } from "react";

import { getFlowRun, listFlowRuns } from "@/lib/flows";
import type { FlowRunSnapshot } from "@/lib/flows";

/** Poll interval while a run is non-terminal. */
const POLL_MS = 300;
/** A hard ceiling on poll attempts so a stuck/in-progress run can never poll forever. */
const MAX_POLLS = 200;

export interface FlowRunState {
  snapshot: FlowRunSnapshot | null;
  error: string | null;
  /** Set the active run id; `null` idles (no polling). */
  watch: (runId: string | null) => void;
  /** On open: find the active run for `flowId` (if any) and begin watching it. */
  reattach: (flowId: string) => Promise<void>;
}

/** Poll a run until it settles. Re-runs cleanly when the run id changes; cancels its loop on
 *  unmount. Returns the snapshot + a `watch(runId)` setter + a `reattach(flowId)` helper. */
export function useFlowRun(): FlowRunState {
  const [runId, setRunId] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<FlowRunSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const cancelled = useRef(false);

  const watch = useCallback((next: string | null) => {
    setRunId(next);
  }, []);

  const reattach = useCallback(async (flowId: string) => {
    try {
      const runs = await listFlowRuns(flowId, "active");
      const active = runs.find((r) => r.status === "active" || r.status === "running");
      setRunId(active?.runId ?? null);
    } catch {
      // No active run is the common case (the flow was idle) — not an error worth surfacing.
      setRunId(null);
    }
  }, []);

  useEffect(() => {
    cancelled.current = false;
    setSnapshot(null);
    setError(null);
    if (!runId) return;

    let polls = 0;
    async function tick() {
      while (!cancelled.current && polls < MAX_POLLS) {
        polls += 1;
        try {
          const snap = await getFlowRun(runId as string);
          if (cancelled.current) return;
          setSnapshot(snap);
          if (snap.status === "success" || snap.status === "partialFailure" || snap.status === "failed") {
            return;
          }
        } catch (e) {
          if (cancelled.current) return;
          setError(e instanceof Error ? e.message : String(e));
          return;
        }
        await new Promise((r) => setTimeout(r, POLL_MS));
      }
    }
    void tick();

    return () => {
      cancelled.current = true;
    };
  }, [runId]);

  return { snapshot, error, watch, reattach };
}
