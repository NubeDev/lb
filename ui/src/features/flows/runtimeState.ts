// A flow's runtime state for the canvas — the PLC answer to "is this flow running?". A flow is a
// long-lived runtime: if it is ENABLED it is RUNNING (the host's reactors scan it every tick and its
// node values advance 24/7), and if it is DISABLED it is STOPPED. That is the ENTIRE model.
//
// The frontend knows NOTHING about node types. It does not enumerate which nodes "fire on their own"
// (cron, flipflop, …) — doing so was the original bug: a per-type guess (`isScheduled`) that the
// frontend had to keep in sync with the backend reactors, and that silently froze the canvas for any
// self-driving node the guess didn't recognise. "Live values" means exactly: while enabled, poll
// `node_state` and paint whatever the host reports. Add a new self-firing node type on the backend and
// it just works here — nothing to update. `enabled` is the only gate.
//
// CRITICAL (the "running-after-restart" report): `enabled` and the schedule detail come from the
// AUTHORITATIVE `flows.node_state` (the live per-trigger cursors), NOT the flow record's dormant
// `cron`/`next_attempt_ts` — those went stale when triggers moved to per-node cursors
// (flow-multi-trigger-reactive-scope). The flow record is only a fallback until node_state loads.
// This pure function is the single source of that truth; the banner renders it and the canvas polls
// on it. (FILE-LAYOUT.)

import type { Flow, FlowNodeState, FlowRunSummary } from "@/lib/flows";

/** The runtime posture the canvas shows. Two states, like a PLC: running (enabled) or stopped. */
export type FlowRunState =
  | "running" // enabled — the runtime is live, reactors advance its values every tick
  | "stopped"; // disabled — nothing fires

export interface FlowRuntimeState {
  state: FlowRunState;
  /** True while the flow is enabled (the runtime is live). Convenience mirror of `state === "running"`. */
  running: boolean;
  /** The cron spec, if the flow has a cron schedule (shown verbatim as detail — never a gate). */
  cron: string | null;
  /** Next scheduled firing instant (unix secs), if the flow has a schedule with a live cursor. */
  nextFireTs: number | null;
  /** The most recent run (newest-first list → `runs[0]`), if any. */
  latestRun: FlowRunSummary | null;
}

/** Derive the runtime state. `nodeState` (from `flows.node_state`) is the AUTHORITATIVE durable source
 *  for `enabled` + the soonest cron/nextAttemptTs; the flow record is only a fallback for before it
 *  loads. `enabled` defaults true (host `enabled_default`). */
export function deriveRuntimeState(
  flow: Flow,
  runs: FlowRunSummary[],
  nodeState?: FlowNodeState | null,
): FlowRuntimeState {
  const latestRun = runs.length > 0 ? runs[0] : null;
  const enabled = nodeState?.enabled ?? flow.enabled ?? true;
  const cron = nodeState?.cron ?? flow.cron ?? null;
  const rawNext = nodeState?.nextAttemptTs ?? flow.nextAttemptTs ?? 0;
  const nextFireTs = rawNext > 0 ? rawNext : null;

  return {
    state: enabled ? "running" : "stopped",
    running: enabled,
    cron,
    nextFireTs,
    latestRun,
  };
}

/** A human "in 33s" / "in 2m" / "any moment" for a future instant, or "—" when unknown. `nowSecs`
 *  is injected so the renderer can tick it and tests are deterministic. */
export function relativeFuture(targetSecs: number | null, nowSecs: number): string {
  if (targetSecs == null) return "—";
  const delta = targetSecs - nowSecs;
  if (delta <= 0) return "any moment";
  if (delta < 60) return `in ${delta}s`;
  const mins = Math.round(delta / 60);
  return `in ${mins}m`;
}

/** A human "30s ago" / "2m ago" for a past instant, or "—" when unknown. */
export function relativePast(targetSecs: number | null | undefined, nowSecs: number): string {
  if (targetSecs == null) return "—";
  const delta = nowSecs - targetSecs;
  if (delta < 0) return "just now";
  if (delta < 60) return `${delta}s ago`;
  const mins = Math.round(delta / 60);
  return `${mins}m ago`;
}
