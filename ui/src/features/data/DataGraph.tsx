// The react-flow graph view (data-console scope) — draws the workspace's records as nodes and their
// real relation edges, so a non-SQL user *follows* a relationship by clicking instead of writing a
// traversal. Lazy-loaded (code-split) so `@xyflow/react` only ships on the Data page. Click a node to
// expand its neighbours (`store.graph(id)`). Layout is a simple deterministic grid — the first cut is
// depth-1 + click-to-expand, not an auto-laid-out whole tenant (scope: keep it small).

import { useMemo } from "react";
import { ReactFlow, Background, type Edge, type Node } from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import type { Graph } from "@/lib/data/data.types";

interface Props {
  graph: Graph;
  /** Expand a node's neighbours on click. */
  onExpand: (id: string) => void;
}

/** Lay nodes out on a deterministic grid (no physics — stable across renders, no wall-clock/random,
 *  testing §3). The id is the react-flow node id directly (already unique). */
function layout(graph: Graph): { nodes: Node[]; edges: Edge[] } {
  const COLS = 4;
  const nodes: Node[] = graph.nodes.map((n, i) => ({
    id: n.id,
    position: { x: (i % COLS) * 220, y: Math.floor(i / COLS) * 120 },
    data: { label: `${n.kind}\n${shortId(n.id)}` },
    style: { fontSize: 11, whiteSpace: "pre-line" as const },
  }));
  const edges: Edge[] = graph.edges.map((e, i) => ({
    id: `${e.source}->${e.target}#${i}`,
    source: e.source,
    target: e.target,
    label: e.label,
  }));
  return { nodes, edges };
}

/** Trim a `table:id` to its id half for the node label (the kind already names the table). */
function shortId(recordId: string): string {
  const i = recordId.indexOf(":");
  return i >= 0 ? recordId.slice(i + 1) : recordId;
}

export default function DataGraph({ graph, onExpand }: Props) {
  const { nodes, edges } = useMemo(() => layout(graph), [graph]);

  return (
    <div className="h-full w-full" data-testid="data-graph">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        fitView
        onNodeClick={(_e, node) => onExpand(node.id)}
        proOptions={{ hideAttribution: true }}
      >
        <Background />
      </ReactFlow>
    </div>
  );
}
