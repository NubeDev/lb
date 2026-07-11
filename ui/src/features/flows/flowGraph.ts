// The canvas ⇄ record serialization (flows-canvas scope, Wave 3 — the named-concept file, NOT a
// utils dump). One responsibility: map a `Flow`'s typed nodes/needs to React Flow nodes/edges and back,
// 1:1, so a save is a faithful serialization and a load a faithful render (no canvas-only state the
// record can't hold). Also maps a run snapshot's per-node outcome → the colour each node paints, and
// derives the executed-node set the editor locks during an active run.

import type { Edge, Node } from "@xyflow/react";

import type {
  Flow,
  FlowNode,
  FlowNodeState,
  FlowRunSnapshot,
  JoinPolicy,
  NodeColour,
  NodeDescriptor,
} from "@/lib/flows";

/** A resolved input port for canvas rendering: its name + the **effective** join policy — `any`
 *  for every port unless the descriptor's `inputPorts` table explicitly declares `all`
 *  (flow-plain-wiring-scope: plain per-message wiring is the universal default). Mirrors
 *  `NodeDescriptor::join_of` on the host. */
export interface CanvasInputPort {
  name: string;
  join: JoinPolicy;
}

/** The effective join policy a port settles under: **`any` for every port, every kind** — plain
 *  per-message wiring (flow-plain-wiring-scope; no per-kind branch). Only an explicit `inputPorts`
 *  entry declaring `all` (a descriptor opt-in; no built-in has one) barriers. `port === undefined`
 *  resolves the primary (first) input port. Mirrors the host's `join_of`. */
export function joinOf(desc: NodeDescriptor, port?: string): JoinPolicy {
  const name =
    port && port !== ""
      ? port
      : desc.inputs[0] ?? desc.inputPorts?.[0]?.name;
  if (!name) return "any";
  return desc.inputPorts?.find((p) => p.name === name)?.join ?? "any";
}

/** The canvas-facing input ports of a descriptor: each declared `inputs[]` port paired with its
 *  effective join policy. Empty for a trigger/source (no input ports ⇒ no target handle rendered). */
export function effectiveInputPorts(desc: NodeDescriptor): CanvasInputPort[] {
  return desc.inputs.map((name) => ({ name, join: joinOf(desc, name) }));
}

/** The data a custom `FlowNodeView` renders: the descriptor type, the live run colour, whether the
 *  node is executed (locked) during the active run, and whether its underlying tool the caller lacks
 *  (shown-but-gated, Decision: the palette reflects permissions). */
export interface FlowNodeData extends Record<string, unknown> {
  type: string;
  /** The descriptor `kind` — the canvas uses it to hide a trigger's target handle (no incoming edge
   *  on an entry node) and to pick affordances. Resolved from the registry, not stored on the node. */
  kind?: import("@/lib/flows").NodeKind;
  /** The descriptor's resolved input ports (flow-input-ports-scope Slice 4) — the canvas renders one
   *  target handle per named port, each with an `any`/`all` glyph. Absent ⇒ the node has no
   *  descriptor yet (a single anonymous primary handle is rendered as the fallback). */
  inputPorts?: CanvasInputPort[];
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

/** A simple left-to-right grid layout for a node index — the FALLBACK used only when a node carries
 *  no stored `position` (a pre-geometry flow, or a node just added by a headless author). Columns of
 *  three keep a small DAG readable until the author drags nodes and saves their geometry. */
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
    // Stored geometry wins; the grid `layout(i)` is only the fallback for a node that never saved a
    // position (a pre-geometry flow). This is what makes a dragged layout persist across reloads.
    position: n.position ?? layout(i),
    data: {
      type: n.type,
      kind: opts.kind?.[n.id],
      colour: opts.colours?.[n.id] ?? "pending",
      locked: opts.locked?.[n.id] ?? false,
      gated: opts.gated?.[n.id] ?? false,
    },
  }));
}

/** Flow → React Flow edges (one per `needs`: source = the dependency, target = the dependent node).
 *  The edge's **target input port** (flow-input-ports-scope Axis 1) rides React Flow's
 *  `targetHandle` (the named port the wire lands on; `null` ⇒ the node's primary input port). A wire
 *  to a NAMED (non-primary) port carries a midpoint `label` so the wire's target is legible without a
 *  separate inspector (flow-input-ports-scope Slice 4) — the common primary-only flow stays clean. */
export function flowToEdges(flow: Flow): Edge[] {
  const edges: Edge[] = [];
  for (const node of flow.nodes) {
    const portByFrom = new Map((node.inputs ?? []).map((w) => [w.from, w.toPort ?? null]));
    for (const dep of node.needs) {
      const toPort = portByFrom.get(dep) ?? null;
      edges.push({
        id: `${dep}->${node.id}`,
        source: dep,
        target: node.id,
        // `targetHandle` carries the named input port (null = primary). React Flow renders handles
        // by id; the canvas names its input handles after the descriptor's input ports.
        targetHandle: toPort,
        // The wire inspector: a named (non-primary) target port shows as a compact midpoint label so
        // a multi-input node's wiring is readable. Primary-only wires stay label-free (clean canvas).
        ...(toPort ? { label: toPort } : {}),
      });
    }
  }
  return edges;
}

/** React Flow nodes + edges → a flow's `nodes[]` (the inverse — a faithful save). Each canvas node
 *  becomes a graph node; each edge `source->target` becomes `target.needs += source` AND, when the
 *  edge carries a non-primary `targetHandle`, a `target.inputs += {from, toPort}` entry (flow-input-
 *  ports-scope Axis 1). Preserves `type`/`config`/`with` from the prior flow (canvas edits topology
 *  + node type; the SchemaForm edits config) and serializes the node's canvas `position` so a dragged
 *  layout persists. */
export function nodesToFlowNodes(
  nodes: FlowCanvasNode[],
  edges: Edge[],
  prior: Flow,
): FlowNode[] {
  const priorById = new Map(prior.nodes.map((n) => [n.id, n]));
  const needsById = new Map<string, string[]>(nodes.map((n) => [n.id, []]));
  /** Per target node: the {from → toPort} map for edges that name a non-primary port. */
  const portByTarget = new Map<string, { from: string; toPort?: string }[]>();
  for (const e of edges) {
    const list = needsById.get(e.target);
    if (list && !list.includes(e.source)) list.push(e.source);
    // `targetHandle` carries the named input port; a null/empty handle ⇒ the primary port (omit the
    // entry so a primary-only flow round-trips to the clean pre-ports shape).
    const handle = typeof e.targetHandle === "string" ? e.targetHandle : null;
    if (handle) {
      let inputs = portByTarget.get(e.target);
      if (!inputs) {
        inputs = [];
        portByTarget.set(e.target, inputs);
      }
      inputs.push({ from: e.source, toPort: handle });
    }
  }
  return nodes.map((n) => {
    const prev = priorById.get(n.id);
    return {
      id: n.id,
      type: n.data.type,
      needs: needsById.get(n.id) ?? [],
      inputs: portByTarget.get(n.id),
      with: prev?.with,
      config: prev?.config,
      // Persist the canvas geometry so a dragged layout survives a save/reload (round its coords to
      // avoid churning the record with sub-pixel drag noise).
      position: { x: Math.round(n.position.x), y: Math.round(n.position.y) },
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

/** A run status that is settled — no live run is in flight. The lock + the Stop button key off the
 *  negation of this (a run is "active" only while NOT terminal). Single source of that truth so the
 *  hook, the canvas, and tests agree. */
export function isTerminalStatus(status: string): boolean {
  return (
    status === "success" ||
    status === "partialFailure" ||
    status === "failed" ||
    status === "cancelled"
  );
}

/** The node ids the config panel must lock RIGHT NOW. The lock applies ONLY while a run is genuinely
 *  in flight (a non-terminal snapshot). A terminal snapshot — a finished manual run, OR the latest
 *  finite firing of an armed cron flow that the canvas latched onto for its values — locks NOTHING:
 *  there is no live run to protect, so the operator edits freely. (Locking on snapshot presence alone
 *  was the bug that left every node read-only after Stop / between firings until a page refresh.) */
export function lockedNodeIds(snap: FlowRunSnapshot | null): Set<string> {
  if (!snap || isTerminalStatus(snap.status)) return new Set<string>();
  return executedNodeIds(snap);
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
    out[s.id] = { output: payloadOf(s.output), error: s.error ?? null };
  }
  return out;
}

/** The value badge a node shows = its envelope's `payload` (flow-message-envelope-scope D10),
 *  falling back to the whole envelope only when there is no `payload` key (a non-envelope value, or a
 *  node that hasn't run). Keeps the canvas legible without the full `{payload, topic, ...}` clutter. */
export function payloadOf(envelope: unknown): unknown {
  if (envelope !== null && typeof envelope === "object" && !Array.isArray(envelope)) {
    const obj = envelope as Record<string, unknown>;
    if ("payload" in obj) return obj.payload;
  }
  return envelope;
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
      out[n.node] = { output: payloadOf(n.value), error: null };
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
