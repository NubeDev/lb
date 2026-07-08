// A custom React Flow node for one typed flow node (flows-canvas scope, Wave 3). Renders the node id,
// its descriptor type, and paints its border by the live run colour (pending → running → ok | err |
// skipped) using theme tokens so it tracks light/dark + accent. The node's last recorded OUTPUT (or
// ERROR) from `flows.runs.get` is shown inline — the run is legible at a glance, the value the
// canvas lacked. During an active run an EXECUTED node is rendered read-only (Decision 1 — the lock
// the v-pinned banner promises); a node whose underlying tool the caller lacks is shown-but-marked
// gated (the palette reflects permissions; the deny still lives at the engine). Handles on both sides
// wire `needs` dependencies — except a `trigger`/`source` (entry) node renders NO target handle, so an
// author cannot wire an incoming edge to a node that has no inputs.

import { Handle, Position, type NodeProps } from "@xyflow/react";

import { cn } from "@/lib/utils";
import { COLOUR_BORDER, COLOUR_DOT, type FlowCanvasNode } from "./flowGraph";
import { nodeIcon } from "./flowIcons";

const STATUS_LABEL: Record<FlowCanvasNode["data"]["colour"], string> = {
  ok: "done",
  err: "error",
  skipped: "skipped",
  running: "running",
  pending: "idle",
};

/** Render a node's output value as a compact, scrollable string. Objects/arrays are JSON; long
 *  strings are clamped by the box's max-height. */
function preview(value: unknown): string {
  if (value === undefined || value === null) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

export function FlowNodeView({ id, data, selected }: NodeProps<FlowCanvasNode>) {
  const out = preview(data.output);
  // Entry nodes (trigger/source) have no input port — render no target handle so they cannot be
  // wired upstream (the descriptor declares no inputs; the canvas honours it).
  const isEntry = data.kind === "trigger" || data.kind === "source";
  const Icon = nodeIcon({ kind: data.kind ?? "transform", icon: undefined });
  return (
    <div
      aria-label={`flow node ${id}`}
      data-colour={data.colour}
      data-locked={data.locked ? "true" : "false"}
      data-gated={data.gated ? "true" : "false"}
      className={cn(
        // The hover/selected treatment conveys state (which node the dock is editing), 150–250ms,
        // transform-free so `prefers-reduced-motion` needs no special-casing (flow-ui-polish).
        "relative w-[180px] rounded-lg border-2 bg-card px-3 py-2 text-xs shadow-sm shadow-black/5 transition-shadow duration-200 hover:shadow-md hover:shadow-black/10",
        COLOUR_BORDER[data.colour],
        selected && "ring-2 ring-accent/50",
        data.locked && "opacity-90",
        data.gated && "opacity-60",
      )}
    >
      {isEntry ? null : (
        <Handle
          type="target"
          position={Position.Left}
          className="!h-2.5 !w-2.5 !border-0 !bg-accent"
        />
      )}
      <div className="flex items-center justify-between gap-2">
        <span className="flex min-w-0 items-center gap-1.5">
          <Icon size={13} className="shrink-0 text-accent" />
          <span className="truncate font-semibold text-fg">{id}</span>
        </span>
        <span className="flex items-center gap-1 text-[10px] uppercase tracking-wide text-muted">
          <span className={cn("h-1.5 w-1.5 rounded-full", COLOUR_DOT[data.colour])} aria-hidden />
          {STATUS_LABEL[data.colour]}
        </span>
      </div>
      <div className="mt-0.5 truncate text-muted">{data.type}</div>

      {data.gated ? (
        <div className="mt-1.5 text-[10px] font-medium text-destructive">gated</div>
      ) : null}
      {data.locked ? (
        <div className="mt-1 text-[10px] text-muted">executed · read-only</div>
      ) : null}

      {data.error ? (
        <div
          aria-label={`node ${id} error`}
          className="mt-1.5 max-h-28 overflow-auto break-words rounded-md bg-destructive/10 px-1.5 py-1 font-mono text-[10px] leading-tight text-destructive"
        >
          {data.error}
        </div>
      ) : out ? (
        <div
          aria-label={`node ${id} output`}
          className="mt-1.5 max-h-28 overflow-auto break-words rounded-md bg-bg/80 px-1.5 py-1 font-mono text-[10px] leading-tight text-fg"
        >
          {out}
        </div>
      ) : null}
      <Handle
        type="source"
        position={Position.Right}
        className="!h-2.5 !w-2.5 !border-0 !bg-accent"
      />
    </div>
  );
}
