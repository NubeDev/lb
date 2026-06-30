// The node palette (flows-canvas scope, Wave 3). Renders the merged `flows.nodes` registry grouped
// by `category` (built-ins + every installed extension's `[[node]]` descriptors). Dragging a node
// type onto the canvas (or clicking it) adds a node instance — a `flow` record edit. A node whose
// underlying tool the caller lacks is SHOWN but marked gated (the menu reflects permissions; the deny
// lives at the engine, never widened — `caller ∩ grant`).

import { useMemo } from "react";

import type { NodeDescriptor } from "@/lib/flows";

interface PaletteProps {
  nodes: NodeDescriptor[];
  /** Add a node instance of this descriptor type to the canvas. */
  onAdd: (desc: NodeDescriptor) => void;
}

/** Group the registry by `category` (stable order: first-seen). Built-ins share the same grouping as
 *  extension nodes — one renderer, no `if native` branch. */
function byCategory(nodes: NodeDescriptor[]): { category: string; items: NodeDescriptor[] }[] {
  const order: string[] = [];
  const map = new Map<string, NodeDescriptor[]>();
  for (const n of nodes) {
    if (!map.has(n.category)) {
      map.set(n.category, []);
      order.push(n.category);
    }
    map.get(n.category)!.push(n);
  }
  return order.map((c) => ({ category: c, items: map.get(c)! }));
}

export function Palette({ nodes, onAdd }: PaletteProps) {
  const groups = useMemo(() => byCategory(nodes), [nodes]);
  return (
    <div
      aria-label="flow palette"
      className="flex w-56 flex-col gap-3 overflow-y-auto border-r border-border p-2"
    >
      {groups.map((g) => (
        <div key={g.category} className="flex flex-col gap-1">
          <div className="text-xs font-semibold text-muted">{g.category}</div>
          {g.items.map((n) => (
            <button
              key={n.type}
              type="button"
              className="rounded-md border border-border bg-bg px-2 py-1 text-left text-xs text-fg hover:border-accent"
              aria-label={`add node ${n.type}`}
              draggable
              onDragStart={(e) => e.dataTransfer.setData("application/x-flow-node", n.type)}
              onClick={() => onAdd(n)}
            >
              <div className="font-medium">{n.title}</div>
              <div className="text-muted">{n.type}</div>
            </button>
          ))}
        </div>
      ))}
      {nodes.length === 0 ? <div className="text-xs text-muted">No nodes.</div> : null}
    </div>
  );
}
