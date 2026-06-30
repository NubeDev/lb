// A custom React Flow node for one typed flow node (flows-canvas scope, Wave 3). Renders the node id,
// its descriptor type, and paints its border by the live run colour (pending → running → ok | err |
// skipped). During an active run an EXECUTED node is rendered read-only (Decision 1 — the lock the
// v-pinned banner promises); a node whose underlying tool the caller lacks is shown-but-marked gated
// (the palette reflects permissions; the deny still lives at the engine). Handles on both sides wire
// `needs` dependencies.

import { Handle, Position, type NodeProps } from "@xyflow/react";

import { COLOUR_HEX, type FlowCanvasNode } from "./flowGraph";

export function FlowNodeView({ id, data }: NodeProps<FlowCanvasNode>) {
  const colour = COLOUR_HEX[data.colour];
  return (
    <div
      aria-label={`flow node ${id}`}
      data-colour={data.colour}
      data-locked={data.locked ? "true" : "false"}
      data-gated={data.gated ? "true" : "false"}
      style={{
        border: `2px solid ${colour}`,
        borderRadius: 8,
        background: data.locked ? "#f3f4f6" : "white",
        opacity: data.gated ? 0.6 : 1,
        padding: "8px 12px",
        minWidth: 130,
        fontSize: 12,
      }}
    >
      <Handle type="target" position={Position.Left} />
      <div style={{ fontWeight: 600 }}>{id}</div>
      <div style={{ color: "#6b7280" }}>{data.type}</div>
      {data.gated ? <div style={{ color: "#dc2626" }}>gated</div> : null}
      {data.locked ? <div style={{ color: "#9ca3af" }}>executed</div> : null}
      <Handle type="source" position={Position.Right} />
    </div>
  );
}
