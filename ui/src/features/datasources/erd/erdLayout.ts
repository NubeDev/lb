// Pure deterministic auto-layout for the schema ERD (datasources-ux ERD scope). No dagre/elkjs — a
// layered column placement: parents (referenced tables) settle left, each referrer one column to the
// right of the table it points at; within a column nodes stack alphabetically. Stable across renders
// (no wall-clock, no random) so it is unit-testable AND identical server/client (testing §3). One
// responsibility, one file (FILE-LAYOUT).

import type { Edge, Node } from "@xyflow/react";

export interface ErdLayoutParams {
  /** Horizontal gap between depth columns, in px. */
  columnWidth?: number;
  /** Vertical gap between stacked nodes within a column, in px. */
  rowHeight?: number;
}

/** Assign each node a deterministic `{x, y}` by layered depth. Inferred edges run child → parent, so a
 *  parent settles at the SMALLEST depth among its referrers and each child sits one column to its right
 *  (`depth[child] = max(depth[child], depth[parent] + 1)`, relaxed over a bounded number of passes). A
 *  cycle can't loop forever — passes are capped at `|nodes|`, so the result is fixed regardless. The
 *  returned positions are ephemeral; the host does NOT persist them (v1). */
export function layoutErd<TData extends Record<string, unknown>>(
  nodes: Node<TData>[],
  edges: Edge[],
  params: ErdLayoutParams = {},
): Node<TData>[] {
  const COL_W = params.columnWidth ?? 340;
  const ROW_H = params.rowHeight ?? 240;

  if (nodes.length === 0) return nodes;

  const known = new Set(nodes.map((n) => n.id));

  // Depth relaxation: parents (edge targets) get a smaller-or-equal depth than their referrers.
  const depth = new Map<string, number>();
  for (const n of nodes) depth.set(n.id, 0);
  for (let pass = 0; pass < nodes.length; pass++) {
    let changed = false;
    for (const e of edges) {
      if (!known.has(e.source) || !known.has(e.target)) continue;
      const parentDepth = depth.get(e.target) ?? 0;
      const want = parentDepth + 1;
      if (want > (depth.get(e.source) ?? 0)) {
        depth.set(e.source, want);
        changed = true;
      }
    }
    if (!changed) break; // settled early (a DAG converges in ≤ |nodes|-1 passes)
  }

  // Group node ids by their depth column, then stack each column alphabetically (deterministic).
  const columns = new Map<number, string[]>();
  for (const n of nodes) {
    const d = depth.get(n.id) ?? 0;
    const col = columns.get(d);
    if (col) col.push(n.id);
    else columns.set(d, [n.id]);
  }
  for (const ids of columns.values()) ids.sort();

  const positioned = nodes.map((n) => ({ ...n, position: { x: 0, y: 0 } }));
  const byId = new Map(positioned.map((n) => [n.id, n]));
  for (const [d, ids] of columns) {
    ids.forEach((id, i) => {
      const node = byId.get(id);
      if (node) node.position = { x: d * COL_W, y: i * ROW_H };
    });
  }
  return positioned;
}
