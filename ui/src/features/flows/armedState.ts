// Derive a flow's RUNTIME state for the canvas banner — "is this flow running?" — from the AUTHORITATIVE
// durable runtime view (`flows.node_state`: `enabled` + the per-trigger cron/nextAttemptTs cursors) +
// its run history. A cron/source flow's runs are each FINITE (one firing → one terminal run), so "is it
// running" is NOT "is there a live run" — it is "is it ARMED (enabled + has a headless trigger) and
// firing on schedule". A manual flow has no headless trigger, so its state is just its last run.
//
// CRITICAL (the "running-after-restart" report): the armed fields come from `node_state`, NOT the
// flow record's `cron`/`next_attempt_ts` — those went dormant when triggers moved to per-node cursors
// (flow-multi-trigger-reactive-scope), so reading them showed an armed cron flow as idle after a
// restart. `node_state` reflects the live per-trigger cursors, so the banner is correct on reload with
// no run in flight. Whether a flow is SCHEDULED (has a headless trigger at all) is read from the GRAPH
// so it holds even when disabled (a disabled cron flow is still "scheduled" — just stopped). This pure
// function is the single source of that truth; the hook + banner render it. (FILE-LAYOUT.)

import type { Flow, FlowNodeState, FlowRunSummary } from "@/lib/flows";

/** Trigger `mode`s that fire WITHOUT a manual Run — the flow runs headless. `manual`/`inject` are
 *  user-driven, so a flow with only those is "idle" (runs on demand), never armed. Mirrors the
 *  `trigger` builtin's mode enum (`flows/src/builtins.rs`). */
const HEADLESS_MODES = new Set(["cron", "event", "boot"]);

/** The runtime posture the canvas shows. */
export type FlowArmedKind =
  | "armed" // enabled + has a cron/source schedule → fires headless on its own
  | "idle" // a manual flow (no schedule) — runs only when you hit Run
  | "disabled"; // explicitly disabled — nothing fires

export interface FlowArmedState {
  kind: FlowArmedKind;
  /** True when the flow fires on a schedule (cron/source) — drives "Deploy/Stop" vs "Run". */
  scheduled: boolean;
  /** The cron spec, if any (shown verbatim). */
  cron: string | null;
  /** Next firing instant (unix secs), if armed + scheduled. */
  nextFireTs: number | null;
  /** The most recent run (newest-first list → `runs[0]`), if any. */
  latestRun: FlowRunSummary | null;
}

/** Does this flow fire on its own (headless)? True when it has ANY trigger node whose mode fires
 *  without a manual Run — cron, event (source), or boot. Read from the GRAPH (not the dormant
 *  `flow.cron`) so it holds even when the flow is disabled: a disabled cron flow is still "scheduled",
 *  just stopped. A flow with only manual/inject triggers is not scheduled (it's idle). */
export function isScheduled(flow: Flow): boolean {
  return flow.nodes.some(
    (n) => n.type === "trigger" && HEADLESS_MODES.has((n.config?.mode as string) ?? "manual"),
  );
}

/** Derive the armed state. `nodeState` (from `flows.node_state`) is the AUTHORITATIVE durable source
 *  for `enabled` + the soonest cron/nextAttemptTs; the flow record is only a fallback for before it
 *  loads. `scheduled` is graph-derived so it's correct for a disabled flow too. */
export function deriveArmedState(
  flow: Flow,
  runs: FlowRunSummary[],
  nodeState?: FlowNodeState | null,
): FlowArmedState {
  const scheduled = isScheduled(flow);
  const latestRun = runs.length > 0 ? runs[0] : null;
  // Authoritative durable runtime fields come from node_state (the per-trigger cursors). Fall back to
  // the flow record only until node_state loads; `enabled` defaults true (host `enabled_default`).
  const enabled = nodeState?.enabled ?? flow.enabled ?? true;
  const cron = nodeState?.cron ?? flow.cron ?? null;
  const rawNext = nodeState?.nextAttemptTs ?? flow.nextAttemptTs ?? 0;
  const nextFireTs = rawNext > 0 ? rawNext : null;

  let kind: FlowArmedKind;
  if (!enabled) {
    kind = "disabled";
  } else if (scheduled) {
    kind = "armed";
  } else {
    kind = "idle";
  }
  return { kind, scheduled, cron, nextFireTs, latestRun };
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
