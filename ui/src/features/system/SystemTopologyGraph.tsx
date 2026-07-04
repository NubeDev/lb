// The react-flow topology view (system-map scope) — draws each subsystem as a node, coloured by its
// live health, and the platform's fixed architectural wiring as edges, so an operator sees *what is
// connected* and *where the failing hop is* in context. Lazy-loaded (code-split) so `@xyflow/react`
// only ships when the user flips to the graph. Layout is a deterministic group-banded grid (no
// physics — stable across renders, no wall-clock/random, testing §3); nodes are read-only (a map, not
// a control panel — Non-goals).

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

import type { Health, SystemTopology } from "@/lib/system/system.types";
import { HEALTH_STYLES } from "./health";

interface Props {
  topology: SystemTopology;
}

interface TopoNodeData extends Record<string, unknown> {
  label: string;
  group: string;
  health: Health;
}

const nodeTypes = { systemNode: SystemNode };

// The order the group bands stack top-to-bottom — mirrors the state-vs-motion split (§3.3).
const GROUP_ORDER = ["motion", "runtime", "state", "workflow"];

/** Lay nodes out on a deterministic grid, banded by `group` (one row per group). Stable across
 *  renders — the id is the react-flow node id directly (already unique within a snapshot). */
function layout(topo: SystemTopology): { nodes: Node<TopoNodeData>[]; edges: Edge[] } {
  const byGroup = new Map<string, string[]>();
  for (const n of topo.nodes) {
    const list = byGroup.get(n.group) ?? [];
    list.push(n.id);
    byGroup.set(n.group, list);
  }
  const groups = [
    ...GROUP_ORDER.filter((g) => byGroup.has(g)),
    ...[...byGroup.keys()].filter((g) => !GROUP_ORDER.includes(g)),
  ];
  const pos = new Map<string, { x: number; y: number }>();
  groups.forEach((g, row) => {
    (byGroup.get(g) ?? []).forEach((id, col) => {
      pos.set(id, { x: col * 220, y: row * 150 });
    });
  });

  const nodes: Node<TopoNodeData>[] = topo.nodes.map((n) => ({
    id: n.id,
    type: "systemNode",
    position: pos.get(n.id) ?? { x: 0, y: 0 },
    data: { label: n.label, group: n.group, health: n.health },
  }));
  const edges: Edge[] = topo.edges.map((e, i) => ({
    id: `${e.from}->${e.to}#${i}`,
    source: e.from,
    target: e.to,
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
      fill: "hsl(var(--muted))",
      fontSize: 11,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
    },
  }));
  return { nodes, edges };
}

function SystemNode({ data }: NodeProps<Node<TopoNodeData>>) {
  const style = HEALTH_STYLES[data.health];
  return (
    <div className={`w-52 rounded-md border bg-bg shadow-sm ${style.border}`}>
      <div className="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
        <span className="min-w-0 truncate text-xs font-medium text-fg">{data.label}</span>
        <span className={`h-2 w-2 shrink-0 rounded-full ${style.dot}`} aria-hidden />
      </div>
      <div className="flex items-center justify-between px-3 py-1.5">
        <span className="font-mono text-[10px] uppercase tracking-wide text-muted">{data.group}</span>
        <span className={`text-[10px] font-medium ${style.text}`}>{style.label}</span>
      </div>
    </div>
  );
}

export default function SystemTopologyGraph({ topology }: Props) {
  const { nodes, edges } = useMemo(() => layout(topology), [topology]);

  if (nodes.length === 0) {
    return (
      <div className="flex h-full items-center justify-center bg-bg p-8 text-center">
        <div className="max-w-sm">
          <div className="mx-auto flex h-10 w-10 items-center justify-center rounded-md border border-border bg-panel">
            <Network size={18} className="text-accent" />
          </div>
          <h2 className="mt-3 text-sm font-medium">No topology yet</h2>
          <p className="mt-1 text-sm text-muted">The snapshot returned no subsystems.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col" data-testid="system-graph">
      <div className="flex min-h-[3.25rem] items-center justify-between gap-3 border-b border-border bg-bg px-3 py-2">
        <div className="min-w-0">
          <div className="text-sm font-medium">Topology</div>
          <div className="truncate text-xs text-muted">
            {nodes.length} subsystem{nodes.length === 1 ? "" : "s"} · {edges.length} connection
            {edges.length === 1 ? "" : "s"}
          </div>
        </div>
        <div className="hidden rounded-md border border-border bg-panel px-2 py-1 text-xs text-muted sm:block">
          Node colour = live health · edges = fixed wiring
        </div>
      </div>

      <div className="min-h-0 flex-1 bg-bg">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          fitView
          nodeTypes={nodeTypes}
          nodesDraggable={false}
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
