// The canvas ⇄ record serialization (flows-canvas scope, Wave 3 — the named-concept file, NOT a
// utils dump). One responsibility: map a `Flow`'s typed nodes/needs to React Flow nodes/edges and back,
// 1:1, so a save is a faithful serialization and a load a faithful render (no canvas-only state the
// record can't hold). Also maps a run snapshot's per-node outcome → the colour each node paints, and
// derives the executed-node set the editor locks during an active run.

import type { Edge, Node } from "@xyflow/react";

import type { Flow, FlowNode, FlowNodeState, FlowRunSnapshot, NodeColour } from "@/lib/flows";

/** The data a custom `FlowNodeView` renders: the descriptor type, the live run colour, whether the
 *  node is executed (locked) during the active run, and whether its underlying tool the caller lacks
 *  (shown-but-gated, Decision: the palette reflects permissions). */
export interface FlowNodeData extends Record<string, unknown> {
  type: string;
  /** The descriptor `kind` — the canvas uses it to hide a trigger's target handle (no incoming edge
   *  on an entry node) and to pick affordances. Resolved from the registry, not stored on the node. */
  kind?: import("@/lib/flows").NodeKind;
  colour: NodeColour;
  /** True once the node has executed in the active run → rendered read-only (Decision 1). */
  locked: boolean;
  /** True when the caller lacks the node's underlying tool cap (shown-but-marked). */
  gated: boolean;
  /** The node's last settled output value (from `flows.runs.get`) — shown on the node so a run
   *  is legible without opening the panel. Undefined when the node hasn't run / produced nothing. */
  output?: unknown;
  /** The node's recorded error text when its outcome is `err`. */
  error?: string | null;
}

export type FlowCanvasNode = Node<FlowNodeData, "flow">;

/** A simple left-to-right grid layout for a node index (the record holds no geometry — the canvas is
 *  a view of the DAG, not a stored layout). Columns of three keep a small DAG readable. */
function layout(index: number): { x: number; y: number } {
  return { x: (index % 3) * 240, y: Math.floor(index / 3) * 130 };
}

/** Flow → React Flow nodes (one per graph node). `colours`/`locked` override the defaults per id. */
export function flowToNodes(
  flow: Flow,
  opts: { colours?: Record<string, NodeColour>; locked?: Record<string, boolean>; gated?: Record<string, boolean>; kind?: Record<string, import("@/lib/flows").NodeKind> } = {},
): FlowCanvasNode[] {
  return flow.nodes.map((n, i) => ({
    id: n.id,
    type: "flow",
    position: layout(i),
    data: {
      type: n.type,
      kind: opts.kind?.[n.id],
      colour: opts.colours?.[n.id] ?? "pending",
      locked: opts.locked?.[n.id] ?? false,
      gated: opts.gated?.[n.id] ?? false,
    },
  }));
}

/** Flow → React Flow edges (one per `needs`: source = the dependency, target = the dependent node). */
export function flowToEdges(flow: Flow): Edge[] {
  const edges: Edge[] = [];
  for (const node of flow.nodes) {
    for (const dep of node.needs) {
      edges.push({ id: `${dep}->${node.id}`, source: dep, target: node.id });
    }
  }
  return edges;
}

/** React Flow nodes + edges → a flow's `nodes[]` (the inverse — a faithful save). Each canvas node
 *  becomes a graph node; each edge `source->target` becomes `target.needs += source`. Preserves
 *  `type`/`config`/`with` from the prior flow (canvas edits topology + node type; the SchemaForm
 *  edits config). */
export function nodesToFlowNodes(
  nodes: FlowCanvasNode[],
  edges: Edge[],
  prior: Flow,
): FlowNode[] {
  const priorById = new Map(prior.nodes.map((n) => [n.id, n]));
  const needsById = new Map<string, string[]>(nodes.map((n) => [n.id, []]));
  for (const e of edges) {
    const list = needsById.get(e.target);
    if (list && !list.includes(e.source)) list.push(e.source);
  }
  return nodes.map((n) => {
    const prev = priorById.get(n.id);
    return {
      id: n.id,
      type: n.data.type,
      needs: needsById.get(n.id) ?? [],
      with: prev?.with,
      config: prev?.config,
    };
  });
}

/** A run snapshot → the colour each node paints. A settled node maps by `outcome`
 *  (ok→green, err→red, skipped→grey); an unsettled node is `running` if claimed past pending, else
 *  `pending`. A Halt-pruned subtree arrives as `skipped` from the host (greyed). */
export function snapshotColours(snap: FlowRunSnapshot): Record<string, NodeColour> {
  const out: Record<string, NodeColour> = {};
  for (const s of snap.steps ?? []) {
    out[s.id] = colourOf(s.outcome ?? null, s.claim);
  }
  return out;
}

/** The set of node ids the editor must LOCK during an active run = the nodes already executed
 *  (`terminal: true` in the snapshot). A config-only `flows.patch_run` may target the rest. */
export function executedNodeIds(snap: FlowRunSnapshot): Set<string> {
  return new Set((snap.steps ?? []).filter((s) => s.terminal).map((s) => s.id));
}

/** A run snapshot → each node's recorded `output`/`error` (the value legibility surface). The host
 *  records these as a node settles; the canvas paints them on the node so the run is readable at a
 *  glance. (`flows.watch` SSE is the named follow-up that streams these live; today the bounded
 *  `flows.runs.get` poll delivers them on each tick.) */
export function snapshotValues(
  snap: FlowRunSnapshot,
): Record<string, { output?: unknown; error?: string | null }> {
  const out: Record<string, { output?: unknown; error?: string | null }> = {};
  for (const s of snap.steps ?? []) {
    out[s.id] = { output: s.output, error: s.error ?? null };
  }
  return out;
}

/** Map the persistent runtime state (`flows.node_state`) to the same per-node `{output}` shape the
 *  canvas paints — so a node shows its CURRENT last-value (Decision 5) even with no run in flight.
 *  A null value (a node that never produced one) is omitted, so an un-run node renders blank, not a
 *  misleading `null`. */
export function nodeStateValues(
  state: FlowNodeState,
): Record<string, { output?: unknown; error?: string | null }> {
  const out: Record<string, { output?: unknown; error?: string | null }> = {};
  for (const n of state.nodes ?? []) {
    if (n.value !== null && n.value !== undefined) {
      out[n.node] = { output: n.value, error: null };
    }
  }
  return out;
}

function colourOf(outcome: string | null, claim?: string): NodeColour {
  switch (outcome) {
    case "ok":
      return "ok";
    case "err":
      return "err";
    case "skipped":
      return "skipped";
    default:
      return claim && claim !== "pending" && claim !== "enqueued" ? "running" : "pending";
  }
}

/** The CSS colour for a node colour (kept for callers that need a literal; the canvas node itself
 *  paints via `COLOUR_BORDER` token classes so it tracks the theme). */
export const COLOUR_HEX: Record<NodeColour, string> = {
  ok: "#16a34a",
  err: "#dc2626",
  skipped: "#9ca3af",
  running: "#f59e0b",
  pending: "#e5e7eb",
};

/** Token-driven Tailwind classes for each run colour — the node border + a status dot track the
 *  outcome AND the active theme (light/dark + accent) without hardcoded hex. Body/labels use the
 *  `bg`/`fg`/`muted` tokens; only the outcome accent is a semantic hue (it carries information). */
export const COLOUR_BORDER: Record<NodeColour, string> = {
  ok: "border-emerald-500/70",
  err: "border-destructive/70",
  skipped: "border-muted/50",
  running: "border-amber-500/80",
  pending: "border-border",
};

/** A small status dot class per colour (the live indicator on each node header). */
export const COLOUR_DOT: Record<NodeColour, string> = {
  ok: "bg-emerald-500",
  err: "bg-destructive",
  skipped: "bg-muted",
  running: "bg-amber-500",
  pending: "bg-border",
};
