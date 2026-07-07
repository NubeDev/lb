// Adapted from Tabularis (github.com/TabularisDB/tabularis), Apache-2.0. Interaction design preserved;
// the data layer is rewired onto our typed SqlBuilderQuery (model-as-truth, not nodes-as-truth).
//
// The join edge — the React-Flow line between two table nodes (visual-canvas-builder slice). Clicking
// the label cycles the `SqlJoinType` INNER→LEFT→RIGHT→FULL→CROSS; the click is delegated to
// `data.onCycle(edgeId)`, which the `QueryCanvas` host wires to a typed `SqlJoin.type` edit on the
// underlying query. One responsibility per file (FILE-LAYOUT): this is only the edge's render + click.

import { BaseEdge, EdgeLabelRenderer, getBezierPath, type EdgeProps } from "@xyflow/react";

import { Button } from "@/components/ui/button";
import type { SqlJoinType } from "@/lib/panel-kit/sql/query";

/** A typed React-Flow edge carrying `JoinEdgeData`. */
export type JoinFlowEdge = EdgeProps & { data?: JoinEdgeData };

/** The data payload `QueryCanvas` puts on each join edge. */
export interface JoinEdgeData extends Record<string, unknown> {
  joinType: SqlJoinType;
  /** Called when the user clicks the label — the host maps the edge id back to the join + cycles it. */
  onCycle?: (edgeId: string) => void;
}

const CYCLE: SqlJoinType[] = ["inner", "left", "right", "full", "cross"];

/** Uppercase display spelling for the join type label. */
export function joinLabel(t: SqlJoinType): string {
  return t.toUpperCase();
}

/** The next type in the cycle (wraps around). */
export function nextJoinType(t: SqlJoinType): SqlJoinType {
  return CYCLE[(CYCLE.indexOf(t) + 1) % CYCLE.length];
}

/** The join edge — a bezier line + a clickable label that cycles the join type. */
export function JoinEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
  markerEnd,
}: JoinFlowEdge) {
  const [edgePath, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  });
  const d = (data ?? {}) as JoinEdgeData;
  const label = joinLabel(d.joinType ?? "inner");

  return (
    <>
      <BaseEdge path={edgePath} markerEnd={markerEnd} />
      <EdgeLabelRenderer>
        <div
          style={{
            position: "absolute",
            transform: `translate(-50%, -50%) translate(${labelX}px,${labelY}px)`,
            pointerEvents: "all",
          }}
          className="nodrag nopan"
        >
          <Button
            type="button"
            variant="ghost"
            onClick={() => d.onCycle?.(id)}
            className="rounded-md border border-border bg-panel px-2 py-0.5 text-[10px] font-medium text-accent shadow transition-colors hover:border-accent hover:bg-bg"
            title="Click to cycle join type"
          >
            {label}
          </Button>
        </div>
      </EdgeLabelRenderer>
    </>
  );
}
