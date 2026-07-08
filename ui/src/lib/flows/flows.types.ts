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
  /** The node's canvas position (editor geometry). Optional — a pre-geometry flow (or a headless
   *  author) omits it and the canvas falls back to a grid layout. Pure view state: it never affects
   *  DAG math, validation, or run order. */
  position?: { x: number; y: number };
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
  /** The next cron firing instant (unix secs), advanced by the reactor (fire-once-then-skip). The
   *  canvas reads it to show "armed · next fire in N" for a cron/source flow. */
  nextAttemptTs?: number;
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
  /** The run's start instant (unix secs). Rows come newest-first, so `runs[0]` is the latest run —
   *  what the canvas paints + dates ("last fired N ago") for an armed flow with no live run. */
  ts?: number;
}

/** One node's current persistent value (Decision 5: `flow_node_state` last-value, updated in place
 *  each scan). `value` is the node's latest output; `rev` bumps on every in-place update. */
export interface NodeStateEntry {
  node: string;
  value: unknown;
  rev: number | null;
  /** The node's node-level RETAINED input (`flow_input:{flow}:{node}`), if any — a control seeds its
   *  current state from its OWN input (not its output `value`). flow-dashboard-binding-ux-scope. */
  input?: unknown;
  /** Per-PORT retained inputs (`flow_input:{flow}:{node}:{port}`), keyed by port name. A per-port
   *  value wins over the node-level `input` for that slot. */
  inputs?: Record<string, unknown>;
}

/** `flows.node_state` — the persistent runtime view: every node's current value + the flow's armed
 *  fields. This is the steady state the canvas paints, independent of any single run. */
export interface FlowNodeState {
  flowId: string;
  enabled?: boolean;
  cron?: string | null;
  nextAttemptTs?: number;
  nodes: NodeStateEntry[];
}

/** A per-node colour the canvas paints from a run snapshot's `outcome`/`claim`. */
export type NodeColour = "ok" | "err" | "skipped" | "running" | "pending";

/** One debug message a `debug` node published onto the per-flow debug subject (debug-node-scope).
 *  The panel renders `value` according to `format` (json → tree, text → <pre>, markdown → rendered),
 *  auto-collapsing when the rendered size exceeds `collapseBytes`. `kind:"dropped"` is the publish-
 *  governor sentinel ("N messages were suppressed under the rate limit") — same attribution, no
 *  `value`. v1 is motion-only: the panel holds these in component state, never persisted. */
export interface DebugMessage {
  kind: "debug" | "dropped";
  /** The debug node id that published (attribution + filter key). */
  node: string;
  /** The run that fired this message (attribution; the stream is per-flow, not per-run). */
  runId?: string;
  /** Unix-ms-ish timestamp from the host (`ts` arg of the run). */
  ts?: number;
  /** `json` | `text` | `markdown` — resolved host-side from the node's `format` config (auto sniffed). */
  format?: "json" | "text" | "markdown";
  /** The wire value the debug node captured. Absent on a `dropped` sentinel. */
  value?: unknown;
  /** The node's label (falls back to the node id) — shown on the row. */
  label?: string;
  /** The node's collapse threshold (bytes); 0 = never collapse. The full value is always on the wire. */
  collapseBytes?: number;
  /** Present (with a count) only on a `dropped` sentinel — the publish governor suppressed this many. */
  dropped?: number;
}

/** True when a run status is terminal (the poll stops here — never an unbounded interval). */
export function isTerminal(status: RunStatus): boolean {
  return status === "success" || status === "partialFailure" || status === "failed";
}
