// React Flow node/edge construction — pure, framework-light helpers extracted
// from CeEditor. These translate the engine's Component/Edge model into the
// @xyflow/react node/edge shapes the canvas renders. No React, no side effects.
import type { Edge as RfEdge, Node as RfNode } from "@xyflow/react";
import { NODE_W, type FunctionBlockData } from "../components/FunctionBlock";
import type { Component, Edge } from "./engine-types";
import { layoutPositions } from "./layout";

const EDGE_TYPE = "smoothstep" as const;

export function buildRfNodes(
  comps: Component[],
  onEnter: (uid: number) => void,
  onContextMenu: (uid: number, x: number, y: number) => void,
  // Carry the existing selection set across reloads — without this, any topology
  // event that fires while a node is selected wipes the selection on the next
  // setNodes call.
  selectedIds?: Set<string>,
  // Component types that declare actions (from /schema) — drives the ⚡ marker.
  actionTypes?: Set<string>,
): RfNode<FunctionBlockData>[] {
  // Position math (grid fallback when unlaid-out + diagonal de-stacking of exact
  // duplicates) lives in lib/layout so it can be tested directly. Display-only:
  // dragging a node persists its real position; a reload re-derives the offsets
  // deterministically from the (stable) REST order.
  const positions = layoutPositions(comps, NODE_W);
  return comps.map((c, i) => {
    const pos = positions[i];
    const id = String(c.uid);
    return {
      id,
      type: "fb",
      position: pos,
      width: NODE_W,
      data: {
        componentUid: c.uid,
        name: c.name,
        hasChildren: (c.childrenCount ?? 0) > 0,
        childCount: c.childrenCount ?? 0,
        hasActions: actionTypes?.has(c.type) ?? false,
        onEnter,
        onContextMenu,
      },
      draggable: true,
      selected: selectedIds?.has(id) ?? false,
    };
  });
}

// MiniMap dot colors — keyed off the same node shape buildRfNodes emits.
// Selected nodes pop in the accent so you can spot your selection on the map.
export function miniMapNodeColor(n: RfNode): string {
  if (n.selected) return "hsl(var(--cool))";
  if (n.type === "ghost") return "hsl(var(--muted-foreground))";
  return (n.data as { hasChildren?: boolean })?.hasChildren ? "hsl(var(--cool))" : "hsl(var(--muted-foreground))";
}
export function miniMapNodeStroke(n: RfNode): string {
  if (n.selected) return "hsl(var(--cool))";
  return (n.data as { hasChildren?: boolean })?.hasChildren ? "hsl(var(--cool))" : "hsl(var(--muted-foreground))";
}

export function buildRfEdges(edges: Edge[], comps: Component[]): RfEdge[] {
  const cByUid = new Map<number, Component>();
  for (const c of comps) cByUid.set(c.uid, c);
  const out: RfEdge[] = [];
  for (const e of edges) {
    const src = cByUid.get(e.sourceUid);
    const dst = cByUid.get(e.targetUid);
    const srcProp = src?.properties[e.sourceProperty];
    const dstProp = dst?.properties[e.targetProperty];
    if (!srcProp || !dstProp) continue;
    // loopBack edges close a feedback cycle and the engine treats them as a one-cycle
    // delay boundary — render them dotted in a muted grey so they read as "logically
    // present but not in the direct dataflow".
    const isLoop = e.loopBack === true;
    out.push({
      id: String(e.uid),
      type: EDGE_TYPE,
      source: String(e.sourceUid),
      sourceHandle: String(srcProp.uid),
      target: String(e.targetUid),
      targetHandle: String(dstProp.uid),
      style: isLoop
        ? { stroke: "hsl(var(--muted-foreground))", strokeWidth: 1.5, strokeDasharray: "6 4" }
        : { stroke: "hsl(var(--cool))", strokeWidth: 1.5 },
      animated: false,
    });
  }
  return out;
}
