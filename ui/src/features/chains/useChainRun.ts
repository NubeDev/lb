// The bounded settle-poll hook (rules-workbench scope, Phase 2 — the RESOLVED poll decision). Given a
// chain id + a run id, poll `chains.runs.get` on a FIXED interval WHILE the status is non-terminal and
// STOP on a terminal status (success/partialFailure/failed). It is NEVER an unbounded setInterval: a
// late open does one snapshot and stops if already terminal. The `chains.watch` SSE is the named
// follow-up that replaces this poll later.

import { useEffect, useRef, useState } from "react";

import { getChainRun, isTerminal, type RunSnapshot } from "@/lib/chains";

/** Poll interval while a run is non-terminal. */
const POLL_MS = 300;
/** A hard ceiling on poll attempts so a stuck/in-progress run can never poll forever. */
const MAX_POLLS = 200;

export interface ChainRunState {
  snapshot: RunSnapshot | null;
  error: string | null;
}

/** Poll a run until it settles. Pass `runId = null` to idle (no polling). Re-runs cleanly when the
 *  run id changes; cancels its loop on unmount. */
export function useChainRun(chainId: string | null, runId: string | null): ChainRunState {
  const [snapshot, setSnapshot] = useState<RunSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const cancelled = useRef(false);

  useEffect(() => {
    cancelled.current = false;
    setSnapshot(null);
    setError(null);
    if (!chainId || !runId) return;

    let polls = 0;
    async function tick() {
      while (!cancelled.current && polls < MAX_POLLS) {
        polls += 1;
        try {
          const snap = await getChainRun(chainId as string, runId as string);
          if (cancelled.current) return;
          setSnapshot(snap);
          if (isTerminal(snap.status)) return; // terminal → stop polling
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
  }, [chainId, runId]);

  return { snapshot, error };
}
