// Derive the LATEST pending run + its terminal signals from a dock session's items (agent-dock scope).
// Pure (FILE-LAYOUT: no React) so the dock's status wiring is unit-testable. A `kind:"agent"` request
// item carries the run `job`; the worker posts a durable `agent_result`/`agent_error` under item id
// `a:<job>`. We find the newest agent request and report whether its result/error has landed yet.

import type { Item } from "@/lib/channel/channel.types";
import { parsePayload } from "@/lib/channel/payload.types";

export interface PendingRun {
  /** The run/job id to watch (the newest agent request's `job`), or null if none was asked yet. */
  job: string | null;
  /** The goal of the newest request (shown while pending). */
  goal: string | null;
  /** True once a durable `agent_result` item (`a:<job>`) exists — the run is Done. */
  hasResult: boolean;
  /** True once a durable `agent_error` item exists for the run — the run is in Error. */
  hasError: boolean;
  /** The error text from a durable `agent_error`, when present. */
  errorText: string | null;
  /** True once a durable `agent_stalled` item exists — the run PAUSED for a keep-going/stop decision.
   *  NOT terminal (the run is Suspended/resumable); the dock renders an actionable prompt. */
  stalled: boolean;
  /** The honest prompt text from a durable `agent_stalled`, when present. */
  stalledText: string | null;
}

const NONE: PendingRun = {
  job: null,
  goal: null,
  hasResult: false,
  hasError: false,
  errorText: null,
  stalled: false,
  stalledText: null,
};

/** Find the newest `kind:"agent"` request and its terminal state. The worker posts the durable answer
 *  as an item whose id is `a:<job>` — an `agent_result` (Done) or `agent_error` (Error). */
export function latestPendingRun(items: Item[]): PendingRun {
  // Newest agent request wins (items are ts-ordered oldest→newest).
  let request: { job: string; goal: string } | null = null;
  for (const it of items) {
    const p = parsePayload(it.body);
    if (p?.kind === "agent") request = { job: p.job, goal: p.goal };
  }
  if (!request) return NONE;

  // The durable answer/error is posted under id `a:<job>`; read its kind to classify the outcome.
  const answer = items.find((it) => it.id === `a:${request.job}`);
  if (answer) {
    const p = parsePayload(answer.body);
    if (p?.kind === "agent_result") {
      return { ...NONE, job: request.job, goal: request.goal, hasResult: true };
    }
    if (p?.kind === "agent_error") {
      return { ...NONE, job: request.job, goal: request.goal, hasError: true, errorText: p.error };
    }
    // PAUSE-AND-ASK: the run stalled and was suspended. Non-terminal — the dock shows keep-going/stop.
    // A later `resume_run` replaces this item with a fresh result/error, so `stalled` naturally clears.
    if (p?.kind === "agent_stalled") {
      return { ...NONE, job: request.job, goal: request.goal, stalled: true, stalledText: p.message };
    }
  }
  return { ...NONE, job: request.job, goal: request.goal };
}
