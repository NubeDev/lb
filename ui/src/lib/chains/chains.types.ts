// The chains wire shapes — mirror the host `Chain`/`Step` records + the `chains.runs.get` snapshot
// (rules-workbench scope, Phase 2). A chain is a DAG of steps over saved rules: `steps` are nodes,
// each step's `needs` are its incoming edges. The canvas maps this 1:1 to React Flow nodes/edges, so
// a save is a faithful serialization and a load a faithful render (no canvas-only state).
//
// The host uses serde camelCase for `Step` (`backoffMs`) and lowercase for the trigger/policy enums.

/** Extra attempts after the first, with a backoff. */
export interface RetrySpec {
  max: number;
  backoffMs?: number;
}

/** One DAG node: a saved rule + its upstream deps + input bindings + optional retry. */
export interface Step {
  id: string;
  /** The saved rule name this step runs (`rule:{ws}:{rule}`). */
  rule: string;
  /** The step ids this step depends on (its incoming edges). */
  needs: string[];
  /** Literal | `${steps.x.output}` | `${params.y}` input bindings. */
  with?: Record<string, unknown>;
  retry?: RetrySpec;
}

/** How a chain run is started (default manual). */
export type Trigger =
  | { kind: "manual" }
  | { kind: "cron"; expr: string }
  | { kind: "event"; topic: string };

/** What happens when a step fails after its retries. `halt` prunes the subtree; `continue` releases
 *  dependents with the failed output resolved to null. */
export type FailurePolicy = "halt" | "continue";

/** A full chain record. `workspace` is set by the host from the token — never sent from the page. */
export interface Chain {
  id: string;
  name: string;
  trigger?: Trigger;
  params?: Record<string, unknown>;
  steps: Step[];
  failure_policy?: FailurePolicy;
}

/** The roster row `chains.list` returns (the host returns full chains; the rail reads id + name). */
export interface ChainSummary {
  id: string;
  name: string;
}

/** A per-step colour the canvas paints from a run snapshot's `outcome`/`claim`. */
export type StepColour = "ok" | "err" | "skipped" | "running" | "pending";

/** One step's slice of a run snapshot (`chains.runs.get`). */
export interface StepSnapshot {
  id: string;
  /** The claim state (`pending`/`claimed`/`done`/...) — lowercased by the host. */
  claim?: string;
  /** `ok` | `err` | `skipped` once settled; absent while pending/running. */
  outcome?: string | null;
  output?: unknown;
  findings?: unknown;
  error?: string | null;
}

/** A run's terminal/in-progress status — camelCase from the host. */
export type RunStatus = "success" | "partialFailure" | "failed" | string;

/** The snapshot the canvas polls to colour nodes as steps settle. */
export interface RunSnapshot {
  run_id: string;
  chain_id: string;
  status: RunStatus;
  steps: StepSnapshot[];
}

/** True when a run status is terminal (the poll stops here — never an unbounded interval). */
export function isTerminal(status: RunStatus): boolean {
  return status === "success" || status === "partialFailure" || status === "failed";
}
