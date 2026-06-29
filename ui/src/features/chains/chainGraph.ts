// The canvas ⇄ record serialization (rules-workbench scope, Phase 2 — the named-concept file, NOT a
// utils dump). One responsibility: map a `Chain`'s steps/needs to React Flow nodes/edges and back,
// 1:1, so a save is a faithful serialization and a load a faithful render (no canvas-only state the
// record can't hold). Also maps a run snapshot's per-step outcome → the colour each node paints.

import type { Edge, Node } from "@xyflow/react";

import type { Chain, RunSnapshot, Step, StepColour } from "@/lib/chains";

/** The data a custom `StepNode` renders: the step id, its rule, retry, and the live run colour. */
export interface StepNodeData extends Record<string, unknown> {
  rule: string;
  retry?: Step["retry"];
  colour: StepColour;
}

export type StepFlowNode = Node<StepNodeData, "step">;

/** A simple left-to-right grid layout for a step index (the record holds no geometry — the canvas is a
 *  view of the DAG, not a stored layout). Columns of three keep a small DAG readable. */
function layout(index: number): { x: number; y: number } {
  return { x: (index % 3) * 220, y: Math.floor(index / 3) * 120 };
}

/** Chain → React Flow nodes (one per step). `colours` overrides the default `pending` per step id. */
export function chainToNodes(
  chain: Chain,
  colours: Record<string, StepColour> = {},
): StepFlowNode[] {
  return chain.steps.map((step, i) => ({
    id: step.id,
    type: "step",
    position: layout(i),
    data: { rule: step.rule, retry: step.retry, colour: colours[step.id] ?? "pending" },
  }));
}

/** Chain → React Flow edges (one per `needs`: source = the dependency, target = the dependent step). */
export function chainToEdges(chain: Chain): Edge[] {
  const edges: Edge[] = [];
  for (const step of chain.steps) {
    for (const dep of step.needs) {
      edges.push({ id: `${dep}->${step.id}`, source: dep, target: step.id });
    }
  }
  return edges;
}

/** React Flow nodes + edges → a chain's `steps[]` (the inverse — a faithful save). Each node becomes a
 *  step; each edge `source->target` becomes `target.needs += source`. Preserves rule/retry from node
 *  data and `with` from the prior chain (canvas edits topology, not bindings). */
export function nodesToSteps(
  nodes: StepFlowNode[],
  edges: Edge[],
  prior: Chain,
): Step[] {
  const priorById = new Map(prior.steps.map((s) => [s.id, s]));
  const needsById = new Map<string, string[]>(nodes.map((n) => [n.id, []]));
  for (const e of edges) {
    const list = needsById.get(e.target);
    if (list && !list.includes(e.source)) list.push(e.source);
  }
  return nodes.map((n) => ({
    id: n.id,
    rule: n.data.rule,
    needs: needsById.get(n.id) ?? [],
    with: priorById.get(n.id)?.with,
    retry: n.data.retry,
  }));
}

/** A run snapshot → the colour each step node paints. A settled step maps by `outcome`
 *  (ok→green, err→red, skipped→grey); an unsettled step is `running` if claimed, else `pending`. A
 *  Halt-pruned subtree arrives as `skipped` from the host (greyed). */
export function snapshotColours(snap: RunSnapshot): Record<string, StepColour> {
  const out: Record<string, StepColour> = {};
  for (const s of snap.steps ?? []) {
    out[s.id] = colourOf(s.outcome ?? null, s.claim);
  }
  return out;
}

function colourOf(outcome: string | null, claim?: string): StepColour {
  switch (outcome) {
    case "ok":
      return "ok";
    case "err":
      return "err";
    case "skipped":
      return "skipped";
    default:
      return claim && claim !== "pending" ? "running" : "pending";
  }
}

/** The CSS colour for a step colour (Tailwind-free so the canvas node can inline it). */
export const COLOUR_HEX: Record<StepColour, string> = {
  ok: "#16a34a",
  err: "#dc2626",
  skipped: "#9ca3af",
  running: "#f59e0b",
  pending: "#e5e7eb",
};
