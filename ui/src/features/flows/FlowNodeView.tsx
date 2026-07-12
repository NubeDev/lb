// A custom React Flow node for one typed flow node (flows-canvas scope, Wave 3). Renders the node id,
// its descriptor type, and paints its border by the live run colour (pending → running → ok | err |
// skipped) using theme tokens so it tracks light/dark + accent. The node's last recorded OUTPUT (or
// ERROR) from `flows.runs.get` is shown inline — the run is legible at a glance, the value the
// canvas lacked. During an active run an EXECUTED node is rendered read-only (Decision 1 — the lock
// the v-pinned banner promises); a node whose underlying tool the caller lacks is shown-but-marked
// gated (the palette reflects permissions; the deny still lives at the engine). Handles on both sides
// wire `needs` dependencies — except a `trigger`/`source` (entry) node renders NO target handle, so an
// author cannot wire an incoming edge to a node that has no inputs.
//
// flow-input-ports-scope Slice 4 + flow-plain-wiring-scope: the target handles are **per named
// input port** (one per declared port). A port is just a port — the default (`any`, plain
// per-message wiring) gets NO glyph; only a port that EXPLICITLY opts into the `all` barrier (an
// extension descriptor; never a built-in) wears the merge glyph. A single-port node keeps the one
// anonymous primary handle (the back-compat shape an edge with no `targetHandle` connects to); a
// multi-port node stacks named handles, each matched by React Flow to the edge's `targetHandle`
// port name.

import { Handle, Position, type NodeProps } from "@xyflow/react";
import { Merge } from "lucide-react";

import { cn } from "@/lib/utils";
import { COLOUR_BORDER, COLOUR_DOT, type FlowCanvasNode, type CanvasInputPort } from "./flowGraph";
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

/** The vertical position (% of node height) for the i-th of `count` input handles — distributes them
 *  evenly so a multi-input node's named handles are visually distinct. */
function handleTop(i: number, count: number): string {
  if (count <= 1) return "50%";
  const step = 100 / (count + 1);
  return `${step * (i + 1)}%`;
}

export function FlowNodeView({ id, data, selected }: NodeProps<FlowCanvasNode>) {
  const out = preview(data.output);
  // Entry nodes (trigger/source) have no input port — render no target handle so they cannot be
  // wired upstream (the descriptor declares no inputs; the canvas honours it).
  const isEntry = data.kind === "trigger" || data.kind === "source";
  const Icon = nodeIcon({ kind: data.kind ?? "transform", icon: undefined });
  // The descriptor's resolved input ports (Slice 4). When absent (no descriptor yet) or a single
  // primary port, fall back to ONE anonymous handle so today's edges (null `targetHandle`) connect.
  const ports: CanvasInputPort[] = data.inputPorts ?? [];
  const singlePrimary = ports.length <= 1;
  return (
    <div
      aria-label={`flow node ${id}`}
      data-colour={data.colour}
      data-locked={data.locked ? "true" : "false"}
      data-gated={data.gated ? "true" : "false"}
      data-ports={ports.map((p) => `${p.name}:${p.join}`).join(",") || undefined}
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
      {isEntry ? null : singlePrimary ? (
        // Single input port (the common case): one anonymous handle (React Flow connects a null
        // `targetHandle` edge to it). No policy glyph — a default (`any`) port is just a port.
        <PortHandle port={ports[0]} top="50%" />
      ) : (
        // Multi-port: the PRIMARY port keeps an anonymous handle (a null `targetHandle` edge lands on
        // it — the canvas convention); each non-primary port carries `id = portName` so its named wire
        // matches, and every handle is labelled with its port name (+ the merge glyph iff explicit-all).
        <>
          {ports.map((p, i) => (
            <PortHandle
              key={p.name}
              port={p}
              top={handleTop(i, ports.length)}
              anonymous={i === 0}
              showLabel
            />
          ))}
        </>
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

/** One input-port target handle. The PRIMARY port (the single port, or the first of a multi-port
 *  node) has NO `id` so an edge with a null `targetHandle` connects to it (the canvas convention);
 *  a non-primary port carries `id = portName` so its named wire matches. Only an EXPLICIT-`all`
 *  port renders the merge glyph (flow-plain-wiring-scope: the `any` default is unmarked — a port
 *  is just a port); (optionally) the port name is labelled beside it. */
function PortHandle({
  port,
  top,
  anonymous,
  showLabel,
}: {
  port: CanvasInputPort | undefined;
  top: string;
  anonymous?: boolean;
  showLabel?: boolean;
}) {
  const explicitAll = port?.join === "all";
  const id = anonymous ? undefined : port?.name;
  return (
    <>
      <Handle
        type="target"
        position={Position.Left}
        id={id}
        style={{ top }}
        className="!h-2.5 !w-2.5 !border-0 !bg-accent"
      />
      {/* Only an explicit-`all` (barrier) port is marked — the merge glyph flags the exception.
          A default (`any`) port renders nothing: plain wiring has no policy to think about. */}
      {explicitAll ? (
        <span
          title="all (join — barrier over this port's upstreams)"
          style={{ top, left: 10 }}
          className="pointer-events-none absolute -translate-y-1/2 text-muted"
          aria-hidden
        >
          <Merge size={10} className="text-muted" />
        </span>
      ) : null}
      {showLabel && port ? (
        <span
          style={{ top, left: explicitAll ? 24 : 10 }}
          className="pointer-events-none absolute -translate-y-1/2 truncate text-[10px] text-muted"
        >
          {port.name}
        </span>
      ) : null}
    </>
  );
}
