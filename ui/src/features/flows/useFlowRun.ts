// The live settle hook (flow-runtime-control-scope — supersedes the bounded poll). Given a run id, it
// prefers the SSE stream (`openFlowRunStream`): one `snapshot` frame seeds the per-node state, then
// each `node-settled`/`run-finished` delta folds in live — so the canvas paints nodes as they settle
// AND a run is observably non-terminal while it runs (which is what makes the Stop button appear and
// live values animate). When no gateway is configured (Tauri/tests) the stream returns null and the
// hook FALLS BACK to the bounded `flows.runs.get` poll (the prior behavior), so nothing regresses.
//
// Also provides `reattach(flowId)` — on open, find the active run via `flows.runs.list
// {status:"active"}` so a canvas reopened mid-run can rejoin it.

import { useCallback, useEffect, useRef, useState } from "react";

import { getFlowRun, listFlowRuns, openFlowRunStream } from "@/lib/flows";
import type { FlowRunSnapshot, FlowStreamEvent, NodeSnapshot } from "@/lib/flows";

/** Poll interval while a run is non-terminal (the no-gateway fallback path only). */
const POLL_MS = 300;
/** A hard ceiling on poll attempts so a stuck/in-progress run can never poll forever. */
const MAX_POLLS = 200;

export interface FlowRunState {
  snapshot: FlowRunSnapshot | null;
  error: string | null;
  /** Set the active run id; `null` idles (no stream/poll). */
  watch: (runId: string | null) => void;
  /** On open: find the active run for `flowId` (if any) and begin watching it. */
  reattach: (flowId: string) => Promise<void>;
}

function isTerminal(status: string): boolean {
  return (
    status === "success" ||
    status === "partialFailure" ||
    status === "failed" ||
    status === "cancelled"
  );
}

/** Fold one settle delta into a snapshot (immutably) — updates the named node's outcome/output, or
 *  sets the terminal run status on `run-finished`. */
function foldEvent(snap: FlowRunSnapshot, ev: FlowStreamEvent): FlowRunSnapshot {
  if (ev.kind === "run-finished") {
    return { ...snap, status: ev.status };
  }
  const steps: NodeSnapshot[] = snap.steps.map((s) =>
    s.id === ev.id
      ? { ...s, claim: "done", terminal: true, outcome: ev.outcome, output: ev.output, error: ev.error ?? null }
      : s,
  );
  return { ...snap, steps };
}

/** Watch a run until it settles — SSE-first, poll-fallback. Re-runs cleanly when the run id changes;
 *  closes its stream / cancels its poll on unmount. */
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

    // Prefer the live SSE stream: a snapshot then folded deltas.
    const stream = openFlowRunStream(
      runId,
      (snap) => {
        if (!cancelled.current) setSnapshot(snap);
      },
      (ev) => {
        if (cancelled.current) return;
        setSnapshot((cur) => (cur ? foldEvent(cur, ev) : cur));
      },
    );
    if (stream) {
      return () => {
        cancelled.current = true;
        stream.close();
      };
    }

    // Fallback (no gateway — Tauri/tests): the bounded `flows.runs.get` poll.
    let polls = 0;
    async function tick() {
      while (!cancelled.current && polls < MAX_POLLS) {
        polls += 1;
        try {
          const snap = await getFlowRun(runId as string);
          if (cancelled.current) return;
          setSnapshot(snap);
          if (isTerminal(snap.status)) return;
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
