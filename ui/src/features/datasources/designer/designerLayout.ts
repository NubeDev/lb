// dagre auto-layout for the schema-designer canvas (schema-designer scope). Wraps `@dagrejs/dagre`
// so the canvas gets a clean left-to-right hierarchical placement (parents left, children right).
// Pure (no React, no IO) → unit-testable. One responsibility, one file (FILE-LAYOUT).
//
// The read-only ERD (`erdLayout.ts`) deliberately avoids dagre (hand-rolled layered). The designer
// opts INTO dagre per the scope ("dagre auto-layout") — a richer layout for an editable canvas
// where the author expects one-click "tidy up". The two layouts coexist (different features).

import Dagre from "@dagrejs/dagre";
import type { Edge, Node } from "@xyflow/react";

/** Run dagre over the given nodes + edges, returning nodes with updated positions. Edges are
 *  passed through unchanged (dagre only positions nodes). Preserves node identity (`id`). */
export function layoutWithDagre<TData extends Record<string, unknown>>(
  nodes: Node<TData>[],
  edges: Edge[],
  options: { rankdir?: "LR" | "TB"; nodeWidth?: number; nodeHeight?: number } = {},
): Node<TData>[] {
  if (nodes.length === 0) return nodes;
  const rankdir = options.rankdir ?? "LR";
  const nodeWidth = options.nodeWidth ?? 280;
  const nodeHeight = options.nodeHeight ?? 200;

  const g = new Dagre.graphlib.Graph();
  g.setGraph({ rankdir, nodesep: 60, ranksep: 100, marginx: 40, marginy: 40 });
  g.setDefaultEdgeLabel(() => ({}));

  for (const n of nodes) {
    // The height tracks the row count so tables with many columns don't overlap their children.
    // A rough heuristic from the node's data columns; the canvas re-flows if the author drags.
    const colCount =
      (n.data as { columns?: unknown[] } | undefined)?.columns?.length ?? 1;
    const h = Math.max(nodeHeight, 40 + colCount * 24);
    g.setNode(n.id, { width: nodeWidth, height: h });
  }
  for (const e of edges) {
    if (nodes.some((n) => n.id === e.source) && nodes.some((n) => n.id === e.target)) {
      g.setEdge(e.source, e.target);
    }
  }
  Dagre.layout(g);

  return nodes.map((n) => {
    const pos = g.node(n.id);
    if (!pos) return n;
    // dagre returns the CENTER; React Flow positions the top-left → offset by half the node box.
    return { ...n, position: { x: pos.x - pos.width / 2, y: pos.y - pos.height / 2 } };
  });
}
