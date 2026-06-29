// A custom React Flow node for one chain step (rules-workbench scope, Phase 2). Renders the step id,
// its rule, an optional retry, and paints its border by the live run colour (pending → running → ok |
// err | skipped). Handles on both sides let an author drag an edge to wire a `needs` dependency.

import { Handle, Position, type NodeProps } from "@xyflow/react";

import { COLOUR_HEX, type StepFlowNode } from "./chainGraph";

export function StepNode({ id, data }: NodeProps<StepFlowNode>) {
  const colour = COLOUR_HEX[data.colour];
  return (
    <div
      aria-label={`step ${id}`}
      data-colour={data.colour}
      style={{
        border: `2px solid ${colour}`,
        borderRadius: 8,
        background: "white",
        padding: "8px 12px",
        minWidth: 120,
        fontSize: 12,
      }}
    >
      <Handle type="target" position={Position.Left} />
      <div style={{ fontWeight: 600 }}>{id}</div>
      <div style={{ color: "#6b7280" }}>{data.rule}</div>
      {data.retry ? <div style={{ color: "#9ca3af" }}>retry ×{data.retry.max}</div> : null}
      <Handle type="source" position={Position.Right} />
    </div>
  );
}
