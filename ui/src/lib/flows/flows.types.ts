// The flows wire shapes — mirror the host `Flow`/`Node` records (`lb-flows/src/model.rs`,
// `#[serde(rename_all = "camelCase")]`) + the `NodeDescriptor` (`descriptor.rs`) + the
// `flows.runs.get` snapshot (`runs.rs`). A flow is a typed, versioned DAG: `nodes` are typed steps,
// each node's `needs` are its incoming edges. The canvas maps this 1:1 to React Flow nodes/edges, so
// a save is a faithful serialization and a load a faithful render (no canvas-only state).
//
// The host uses camelCase for `Node`/`Flow` (`needs`, `failurePolicy`) and lowercase for the
// `NodeKind`/`FailurePolicy` enums. The descriptor's `type` field is serde-renamed (`r#type` in Rust)
// and arrives on the wire as `type`.

/** A coarse node class — palette grouping + wiring affordances. The descriptor carries it; the node
 *  record does not (single source of truth — the descriptor is the join). */
export type NodeKind = "trigger" | "transform" | "sink" | "source";

/** One node descriptor — the join between the editor (palette + schema form) and the engine. Built-ins
 *  (`trigger`/`tool`/`rhai`/`subflow`/`sink`) and extension (`<ext>.<type>`) descriptors share one
 *  shape (Decision: one registry, one renderer, no `if native` branch). */
export interface NodeDescriptor {
  /** Globally-unique node type: a built-in or `<ext>.<type>`. */
  type: string;
  title: string;
  category: string;
  kind: NodeKind;
  /** A lucide icon name the palette + node render (e.g. "zap"). Absent → fall back by `kind`. */
  icon?: string;
  /** The MCP tool this node dispatches (`rules.eval`, `<ext>.<tool>`, …). Engine gates it. */
  tool: string;
  inputs: string[];
  outputs: string[];
  configVersion: number;
  /** Inline JSON-Schema 2020-12 the SchemaForm renders + the host validates saved config against. */
  config: Record<string, unknown>;
}

/** One flow node — a data-driven step. `type` keys into the merged registry; `config` is the validated
 *  instance of that descriptor's schema; `needs` + `with` are the DAG edges + bindings. */
export interface FlowNode {
  id: string;
  /** The descriptor type: a built-in (`trigger`/`tool`/`rhai`/`subflow`/`sink`) or `<ext>.<type>`. */
  type: string;
  needs: string[];
  /** Input bindings: literal | `${steps.x.output}` | `${steps.x.findings}` | `${params.y}`. */
  with?: Record<string, unknown>;
  /** The node's config, validated against its descriptor's schema at save. */
  config?: Record<string, unknown> | null;
}

/** What happens when a node fails after retries. `halt` prunes the subtree; `continue` releases
 *  dependents with the failed output resolved to null. */
export type FailurePolicy = "halt" | "continue";

/** A flow — a typed, versioned node graph. `version` is monotonic; a run pins it (Decision 1).
 *  `workspace` is set host-side from the token — never sent from the page. */
export interface Flow {
  workspace?: string;
  id: string;
  name: string;
  /** Monotonic graph version. A run pins this (Decision 1). */
  version: number;
  params?: Record<string, unknown>;
  nodes: FlowNode[];
  failurePolicy?: FailurePolicy;
  // lifecycle / trigger fields (triggers-lifecycle scope) — additive serde defaults.
  enabled?: boolean;
  startOnBoot?: boolean;
  placement?: "either" | "cloud-only" | "local-only";
  cron?: string | null;
}

/** The roster row `flows.list` returns (the picker). */
export interface FlowSummary {
  id: string;
  name: string;
  version: number;
  nodes: number;
}

/** The result of `flows.save` — the persisted id + the (possibly new) version. */
export interface SaveFlowResult {
  id: string;
  version: number;
}

/** The result of `flows.run` — the durable run id the canvas polls / reattaches to. */
export interface RunFlowResult {
  run_id: string;
}

/** The result of `flows.inject` — whether a one-shot run was fired (Decision 9 retain-vs-fire). */
export interface InjectFlowResult {
  fired_run: boolean;
}

/** One node's slice of a run snapshot (`flows.runs.get`). `claim` lowercased by the host. */
export interface NodeSnapshot {
  id: string;
  /** `pending`/`enqueued`/`running`/`done`. */
  claim?: string;
  /** `true` once the node has executed (the executed-node-lock surface). */
  terminal?: boolean;
  /** `ok` | `err` | `skipped` once settled; absent while pending/running. */
  outcome?: string | null;
  output?: unknown;
  error?: string | null;
}

/** A run's terminal/in-progress status. */
export type RunStatus = "success" | "partialFailure" | "failed" | "running" | string;

/** The snapshot the canvas polls to colour nodes as they settle + drives the executed-node-lock. */
export interface FlowRunSnapshot {
  runId: string;
  flowId: string;
  flowVersion: number;
  status: RunStatus;
  steps: NodeSnapshot[];
}

/** A row of `flows.runs.list` (the reattach surface). */
export interface FlowRunSummary {
  runId: string;
  flowId: string;
  flowVersion: number;
  status: string;
}

/** A per-node colour the canvas paints from a run snapshot's `outcome`/`claim`. */
export type NodeColour = "ok" | "err" | "skipped" | "running" | "pending";

/** True when a run status is terminal (the poll stops here — never an unbounded interval). */
export function isTerminal(status: RunStatus): boolean {
  return status === "success" || status === "partialFailure" || status === "failed";
}
