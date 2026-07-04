// The react-flow graph view (data-console scope) — draws the workspace's records as nodes and their
// real relation edges, so a non-SQL user *follows* a relationship by clicking instead of writing a
// traversal. Lazy-loaded (code-split) so `@xyflow/react` only ships on the Data page. Click a node to
// expand its neighbours (`store.graph(id)`). Layout is a simple deterministic grid — the first cut is
// depth-1 + click-to-expand, not an auto-laid-out whole tenant (scope: keep it small).

import { useMemo } from "react";
import {
  Background,
  Controls,
  ReactFlow,
  type Edge,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { Network } from "lucide-react";

import type { Graph, GraphNode } from "@/lib/data/data.types";

interface Props {
  graph: Graph;
  /** Expand a node's neighbours on click. */
  onExpand: (id: string) => void;
}

interface DataNode extends Record<string, unknown> {
  id: string;
  kind: string;
  shortId: string;
}

const nodeTypes = { dataRecord: DataRecordNode };

/** Lay nodes out on a deterministic grid (no physics — stable across renders, no wall-clock/random,
 *  testing §3). The id is the react-flow node id directly (already unique). */
function layout(graph: Graph): { nodes: Node<DataNode>[]; edges: Edge[] } {
  const COLS = 4;
  const nodes: Node<DataNode>[] = graph.nodes.map((n, i) => ({
    id: n.id,
    type: "dataRecord",
    position: { x: (i % COLS) * 260, y: Math.floor(i / COLS) * 140 },
    data: { id: n.id, kind: n.kind, shortId: shortId(n.id) },
  }));
  const edges: Edge[] = graph.edges.map((e, i) => ({
    id: `${e.source}->${e.target}#${i}`,
    source: e.source,
    target: e.target,
    label: e.label,
    type: "smoothstep",
    animated: false,
    style: { stroke: "hsl(var(--border))", strokeWidth: 1.4 },
    labelBgPadding: [6, 4],
    labelBgBorderRadius: 4,
    labelBgStyle: {
      fill: "hsl(var(--bg))",
      fillOpacity: 0.92,
      stroke: "hsl(var(--border))",
      strokeWidth: 1,
    },
    labelStyle: {
      fill: "hsl(var(--accent))",
      fontSize: 11,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
    },
  }));
  return { nodes, edges };
}

/** Trim a `table:id` to its id half for the node label (the kind already names the table). */
function shortId(recordId: string): string {
  const i = recordId.indexOf(":");
  return i >= 0 ? recordId.slice(i + 1) : recordId;
}

function DataRecordNode({ data, selected }: NodeProps<Node<DataNode>>) {
  return (
    <div
      className={`w-60 rounded-md border bg-bg shadow-sm transition-colors ${
        selected ? "border-accent/70 ring-2 ring-accent/20" : "border-border"
      }`}
    >
      <div className="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
        <span className="min-w-0 truncate font-mono text-xs font-medium text-accent">
          {data.kind}
        </span>
        <span className="rounded-md border border-border bg-panel px-1.5 py-0.5 text-[10px] text-muted">
          record
        </span>
      </div>
      <div className="px-3 py-2">
        <div className="truncate font-mono text-xs text-fg">{data.shortId}</div>
        <div className="mt-1 truncate font-mono text-[10px] text-muted">{data.id}</div>
      </div>
    </div>
  );
}

function EmptyGraph() {
  return (
    <div className="flex h-full items-center justify-center bg-bg p-8 text-center">
      <div className="max-w-sm">
        <div className="mx-auto flex h-10 w-10 items-center justify-center rounded-md border border-border bg-panel">
          <Network size={18} className="text-accent" />
        </div>
        <h2 className="mt-3 text-sm font-medium">No relation graph yet</h2>
        <p className="mt-1 text-sm text-muted">
          The selected table did not return graph nodes. Try another table or expand records after
          relations are written.
        </p>
      </div>
    </div>
  );
}

function graphKinds(nodes: GraphNode[]): string {
  const kinds = [...new Set(nodes.map((n) => n.kind))];
  if (kinds.length === 0) return "0 kinds";
  if (kinds.length <= 3) return kinds.join(", ");
  return `${kinds.slice(0, 3).join(", ")} +${kinds.length - 3}`;
}

export default function DataGraph({ graph, onExpand }: Props) {
  const { nodes, edges } = useMemo(() => layout(graph), [graph]);

  if (nodes.length === 0) return <EmptyGraph />;

  return (
    <div className="flex h-full w-full flex-col" data-testid="data-graph">
      <div className="flex min-h-[3.25rem] items-center justify-between gap-3 border-b border-border bg-bg px-3 py-2">
        <div className="min-w-0">
          <div className="text-sm font-medium">Relation graph</div>
          <div className="truncate text-xs text-muted">
            {nodes.length} node{nodes.length === 1 ? "" : "s"} · {edges.length} edge
            {edges.length === 1 ? "" : "s"} · {graphKinds(graph.nodes)}
          </div>
        </div>
        <div className="hidden rounded-md border border-border bg-panel px-2 py-1 text-xs text-muted sm:block">
          Click a node to expand neighbours
        </div>
      </div>

      <div className="min-h-0 flex-1 bg-bg">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          fitView
          nodeTypes={nodeTypes}
          nodesDraggable={false}
          onNodeClick={(_e, node) => onExpand(node.id)}
          proOptions={{ hideAttribution: true }}
        >
          <Background color="hsl(var(--muted))" gap={40} size={1} />
          <Controls
            showInteractive={false}
            className="overflow-hidden rounded-md border border-border bg-panel text-fg"
          />
        </ReactFlow>
      </div>
    </div>
  );
}
