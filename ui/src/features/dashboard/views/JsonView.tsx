// The `jsonview` read view — pretty-prints a flow node's structured `payload` back out (flow-
// dashboard-binding-ux-scope, "Structured JSON out"). The one genuinely missing read view (built
// ones: chart/stat/gauge/table/scripted/control). It reads the node's OUTPUT value via
// `flows.node_state` (instant + canvas-cadence refresh — NOT `runs.get`, NOT a series watch), defaults
// to the envelope's `payload` field, and renders it collapsible. A denied/empty read degrades to an
// honest empty state — never a fake value.

import { useState } from "react";

import { WidgetHeader, WidgetMessage } from "../widgets/chrome";
import { useFlowNodeValue } from "./useFlowNodeValue";
import { flowBindingOfSource } from "./flowBinding";
import { valueFieldOptions } from "./field";
import { applyMappings } from "../fieldconfig/mappings";
import { resolveColor } from "../fieldconfig/color";
import type { Cell, Source } from "@/lib/dashboard";

interface Props {
  source?: Source;
  options?: Record<string, unknown>;
  label?: string;
  refreshKey?: number;
  /** The full cell — carries `fieldConfig` so a SCALAR value honors value mappings (false→on, etc.). */
  cell?: Cell;
}

export function JsonView({ source, options, label, refreshKey = 0, cell }: Props) {
  const flow = flowBindingOfSource(source);
  // Read views default to the `payload` field (flow-dashboard-binding-ux-scope); `options.envelope`
  // opts into showing the WHOLE `{payload, topic, …}` envelope. Both ride one `flows.node_state` read.
  // A picked JSON path (the visual builder, incl. "(whole value)") extracts exactly that field;
  // otherwise `options.envelope` shows the WHOLE envelope, else the port's `payload`. One read.
  const hasPath = flow?.path !== undefined;
  const wantEnvelope = !hasPath && options?.envelope === true;
  const { value, loading, denied } = useFlowNodeValue(
    flow?.flowId,
    flow?.node,
    flow?.port ?? "payload",
    wantEnvelope ? "output-envelope" : "output",
    refreshKey,
    flow?.path,
  );
  const [collapsed, setCollapsed] = useState(false);

  // Value mappings (false→on, 1→"ok", …) apply to a SCALAR value only — an object/array renders as
  // raw JSON. When a mapping hits, its `text` replaces the raw scalar and its `color` tints it, the
  // same fieldConfig bridge stat/gauge use (never a re-implemented match here). No cell / no mapping /
  // non-scalar value ⇒ the raw pretty-print, unchanged.
  const isScalar = value == null || typeof value !== "object";
  const mapped = cell && isScalar ? applyMappings(value, valueFieldOptions(cell).mappings) : null;

  return (
    <div className="flex h-full flex-col" aria-label={`json view ${flow?.node ?? ""}`}>
      <div className="flex items-center justify-between gap-2">
        <WidgetHeader label={label ?? flow?.node ?? "json"} />
        {/* eslint-disable-next-line no-restricted-syntax -- a tiny inline disclosure, not a Button */}
        <button
          type="button"
          aria-label={collapsed ? "expand" : "collapse"}
          onClick={() => setCollapsed((c) => !c)}
          className="text-xs text-muted hover:text-fg"
        >
          {collapsed ? "▸" : "▾"}
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-auto p-2">
        {loading ? (
          <span className="text-xs text-muted">…</span>
        ) : denied ? (
          <WidgetMessage tone="denied">binding broken — re-pick</WidgetMessage>
        ) : collapsed ? (
          <span className="text-xs text-muted">{mapped?.text ?? summarize(value)}</span>
        ) : mapped ? (
          <span
            aria-label="json content"
            className="font-mono text-xs"
            style={{ color: mapped.color ? resolveColor(mapped.color) : "hsl(var(--fg))" }}
          >
            {mapped.text}
          </span>
        ) : (
          <pre aria-label="json content" className="whitespace-pre-wrap break-words font-mono text-xs text-fg">
            {pretty(value)}
          </pre>
        )}
      </div>
    </div>
  );
}

function pretty(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}

function summarize(v: unknown): string {
  if (v == null) return "null";
  if (Array.isArray(v)) return `[${v.length} items]`;
  if (typeof v === "object") return `{${Object.keys(v as object).length} keys}`;
  return String(v);
}
