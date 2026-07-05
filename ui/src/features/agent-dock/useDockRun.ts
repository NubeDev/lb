// The dock's live run watcher (agent-dock scope) — folds the run-event SSE stream for ONE pending run
// into the six-state feedback contract, honestly degrading when the caller lacks `mcp:agent.watch:call`
// (the stream 403s → no live deltas, the durable answer still renders). Composes the shipped `fold`
// (useRunFeed) for the event math and adds what the dock needs beyond the channel card: a stall clock
// input (last-event time), a transport-error flag, and the folded phase.
//
// FILE-LAYOUT: one hook per file. The phase reducer is `dockRunState.ts`; the stall math is
// `useStallTimer.ts`; this hook wires the live stream to both.

import { useEffect, useMemo, useRef, useState } from "react";

import { fold, type RunFeed } from "@/features/channel/useRunFeed";
import { openRunStream } from "@/lib/channel/run.stream";
import { dockRunPhase, type DockRunPhase } from "./dockRunState";
import { useStallTimer } from "./useStallTimer";

const EMPTY: RunFeed = { live: false, text: "", reasoning: "", tools: [], finished: false };

/** Module-level default clock — a stable identity so it does NOT re-trigger `useStallTimer`'s effect
 *  on every render (an inline `() => Date.now()` default would). Tests inject their own `now`. */
const SYSTEM_NOW = () => Date.now();

export interface DockRun {
  phase: DockRunPhase;
  feed: RunFeed;
  /** Whole seconds since the run started (shown while Working/Answering/Stalled). */
  elapsedSec: number;
  /** True when the live progress stream failed to open / was denied (no `agent.watch`) — the dock
   *  shows a "no live progress" notice but still renders the durable answer (honest degrade). */
  degraded: boolean;
}

/** Watch pending run `job` while `active` (no durable result/error yet). `hasResult`/`hasError` are the
 *  terminal signals derived from channel history (a durable `agent_result` / `agent_error` item, or a
 *  transport failure). `now` is injectable for tests. */
export function useDockRun(
  job: string,
  active: boolean,
  hasResult: boolean,
  hasError: boolean,
  now: () => number = SYSTEM_NOW,
): DockRun {
  const [feed, setFeed] = useState<RunFeed>(EMPTY);
  const [streamError, setStreamError] = useState(false);
  // Timestamps drive the stall clock: when the run started, and when the last event arrived.
  const startedAt = useRef<number | null>(null);
  const [lastEventAt, setLastEventAt] = useState<number | null>(null);

  useEffect(() => {
    if (!active) return;
    startedAt.current = now();
    setFeed(EMPTY);
    setStreamError(false);
    setLastEventAt(null);
    const stream = openRunStream(
      job,
      (event) => {
        setFeed((prev) => fold(prev, event));
        setLastEventAt(now());
      },
      () => setStreamError(true),
    );
    if (stream) setFeed((prev) => ({ ...prev, live: true }));
    return () => stream?.close();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `now` is stable; re-subscribe on job/active only.
  }, [job, active]);

  const stall = useStallTimer(active ? startedAt.current : null, lastEventAt, active && !hasResult, now);

  // A watch-stream error is a DEGRADE, not a hard Error: it means "no live deltas" (e.g. the caller
  // lacks `mcp:agent.watch:call`, or the stream dropped) — the durable `agent_result` still arrives on
  // the channel. So `streamError` NEVER feeds `hasError`; the Error phase comes ONLY from a durable
  // `agent_error` item or a channel post/auth rejection (`hasError`), per the scope's degrade rule.
  const phase = useMemo(
    () =>
      dockRunPhase({
        feed: startedAt.current != null ? feed : null,
        hasResult,
        hasError,
        stalled: stall.stalled,
      }),
    [feed, hasResult, hasError, stall.stalled],
  );

  // Degraded whenever the live stream failed but the run is not itself in Error — the dock shows a
  // "no live progress" notice and leans on the durable answer.
  const degraded = streamError && phase !== "error";

  return { phase, feed, elapsedSec: stall.elapsedSec, degraded };
}
