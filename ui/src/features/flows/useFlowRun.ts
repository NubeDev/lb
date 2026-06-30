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
import type { FlowRunSnapshot, FlowRunSummary, FlowStreamEvent, NodeSnapshot } from "@/lib/flows";

/** Poll interval while a run is non-terminal (the no-gateway fallback path only). */
const POLL_MS = 300;
/** A hard ceiling on poll attempts so a stuck/in-progress run can never poll forever. */
const MAX_POLLS = 200;

export interface FlowRunState {
  snapshot: FlowRunSnapshot | null;
  error: string | null;
  /** The flow's runs, newest-first (the armed-state banner reads `runs[0]` as the latest run). */
  runs: FlowRunSummary[];
  /** Set the active run id; `null` idles (no stream/poll). */
  watch: (runId: string | null) => void;
  /** On open: load the flow's runs and begin watching the MOST RECENT one (active or terminal) so an
   *  armed cron flow shows its latest values + "last fired" even with no live run in flight. */
  reattach: (flowId: string) => Promise<void>;
  /** Re-poll the runs list (cheap) so a new cron firing surfaces in the banner without reopening. */
  refreshRuns: (flowId: string) => Promise<void>;
  /** Optimistically mark the watched run terminal (e.g. right after a Stop/cancel) so the canvas's
   *  run-active lock releases IMMEDIATELY — without waiting on the SSE `run-finished` frame, which
   *  may never arrive if the stream already closed. The host stays the source of truth on next load. */
  markTerminal: (status: string) => void;
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
  const [runs, setRuns] = useState<FlowRunSummary[]>([]);
  const cancelled = useRef(false);

  const watch = useCallback((next: string | null) => {
    setRunId(next);
  }, []);

  const markTerminal = useCallback((status: string) => {
    setSnapshot((cur) => (cur ? { ...cur, status } : cur));
  }, []);

  const reattach = useCallback(async (flowId: string) => {
    try {
      // ALL runs, newest-first (host sorts by ts desc). For a cron/source flow each run is finite, so
      // there is usually no "active" run between firings — but the LATEST run carries the most recent
      // values and a "fired N ago" timestamp. Watching it paints those values; if it happens to still
      // be running, the SSE stream folds it live. This is why the canvas no longer looks dead between
      // cron firings.
      const all = await listFlowRuns(flowId);
      setRuns(all);
      const active = all.find((r) => r.status === "active" || r.status === "running");
      setRunId((active ?? all[0])?.runId ?? null);
    } catch {
      setRuns([]);
      setRunId(null);
    }
  }, []);

  /** Re-poll the runs list (the armed-state banner's "last fired" + count). Lightweight — used by a
   *  slow interval while a flow is armed so a new cron firing shows without reopening the flow. */
  const refreshRuns = useCallback(async (flowId: string) => {
    try {
      const all = await listFlowRuns(flowId);
      if (!cancelled.current) setRuns(all);
    } catch {
      /* transient — keep the last list */
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

  return { snapshot, error, runs, watch, reattach, refreshRuns, markTerminal };
}
