// The stall-timer state machine (agent-dock scope, feedback state 4) — tracks elapsed run time and
// flips a `stalled` flag when the run stream has gone quiet for too long. A stall is NOT an error (the
// server-side 15-min wall ceiling posts an honest agent_error on a true timeout); it is an honest
// "still working" hint so a slow-but-alive agent never reads as a dead spinner.
//
// FILE-LAYOUT: one hook per file. The timing is a pure reducer over `(now, lastEventAt)` so the state
// machine is unit-testable without fake timers; the hook wraps it with a real interval.

import { useEffect, useRef, useState } from "react";

/** No `RunEvent` for this long (ms) with the run still live ⇒ `stalled`. Scope: 15 s. */
export const STALL_AFTER_MS = 15_000;

/** How often the hook re-evaluates elapsed/stall while active (ms). Cheap — a single 1 s tick. */
const TICK_MS = 1_000;

/** Module-level default clock so the hook's `useEffect` dep stays stable across renders (an inline
 *  `() => Date.now()` default would mint a fresh identity each render and re-trigger the effect →
 *  infinite setState loop). Tests inject their own `now`. */
const SYSTEM_NOW = () => Date.now();

export interface StallState {
  /** Whole seconds since the run started — the elapsed timer the card shows. */
  elapsedSec: number;
  /** True when the run is live but no event arrived for `STALL_AFTER_MS` — a hint, not an error. */
  stalled: boolean;
}

/** Pure state-machine step: given the run start, the last-event time, and now, compute the state.
 *  Exported for unit testing (no timers). `stalled` requires a live run (started, not finished) that
 *  has been quiet past the threshold; a finished/absent run is never stalled. */
export function computeStall(
  startedAt: number | null,
  lastEventAt: number | null,
  active: boolean,
  now: number,
): StallState {
  if (startedAt == null || !active) return { elapsedSec: 0, stalled: false };
  const elapsedSec = Math.max(0, Math.floor((now - startedAt) / 1000));
  const since = lastEventAt ?? startedAt;
  return { elapsedSec, stalled: now - since >= STALL_AFTER_MS };
}

/** Track elapsed + stall for a run that is `active` (pending, no durable answer yet). `startedAt` is
 *  when the run began (send time); `lastEventAt` bumps on every stream event (pass the feed's latest
 *  event timestamp). Stops ticking when `active` is false. `now` is injectable for tests. */
export function useStallTimer(
  startedAt: number | null,
  lastEventAt: number | null,
  active: boolean,
  now: () => number = SYSTEM_NOW,
): StallState {
  const [state, setState] = useState<StallState>(() =>
    computeStall(startedAt, lastEventAt, active, now()),
  );
  // Keep the latest inputs in a ref so the interval callback always sees current values without
  // re-subscribing every tick (a fresh interval each render would drift the cadence).
  const inputs = useRef({ startedAt, lastEventAt, active });
  inputs.current = { startedAt, lastEventAt, active };

  useEffect(() => {
    setState(computeStall(startedAt, lastEventAt, active, now()));
    if (!active || startedAt == null) return;
    const id = setInterval(() => {
      const { startedAt: s, lastEventAt: l, active: a } = inputs.current;
      setState(computeStall(s, l, a, now()));
    }, TICK_MS);
    return () => clearInterval(id);
    // `lastEventAt` in deps re-seeds immediately on a new event (resets the stall clock) without
    // waiting for the next tick; `now` is stable.
  }, [startedAt, lastEventAt, active, now]);

  return state;
}
