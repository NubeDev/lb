// The `jsonview` read view — pretty-prints a structured value (flow-dashboard-binding-ux-scope,
// "Structured JSON out", widened for descriptor-declared tool renders). Two source kinds, one view:
//   - a FLOW binding (`flows.node_state`) reads the node's OUTPUT value via `useFlowNodeValue`
//     (instant + canvas-cadence refresh — NOT `runs.get`, NOT a series watch), defaulting to the
//     envelope's `payload` field;
//   - any OTHER tool source (e.g. a `federation.sample` rich_result envelope) resolves through the
//     ONE panel-data hook (`usePanelData` → viz.query), so the view stays inside invariant A.
// `options.collapsed: true` starts the view COLLAPSED to a one-line summary — how a large payload
// (a datasource snapshot) lands in a channel without flooding it; the reader expands on demand.
// A denied/empty read degrades to an honest empty state — never a fake value.

import { useState } from "react";

import { WidgetHeader, WidgetMessage } from "../widgets/chrome";
import { useFlowNodeValue } from "./useFlowNodeValue";
import { flowBindingOfSource } from "./flowBinding";
import { valueFieldOptions } from "./field";
import { applyMappings } from "../fieldconfig/mappings";
import { resolveColor } from "../fieldconfig/color";
import { usePanelData } from "../builder/usePanelData";
import type { Cell, Source } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";

interface Props {
  source?: Source;
  options?: Record<string, unknown>;
  label?: string;
  refreshKey?: number;
  /** The full cell — carries `fieldConfig` so a SCALAR value honors value mappings (false→on, etc.),
   *  and the source targets for the non-flow (tool) read path. */
  cell?: Cell;
  scope?: VarScope;
}

/** A no-target placeholder so the inactive `usePanelData` makes no real call when the flow path (or
 *  no cell at all) owns the view — both hooks stay unconditionally mounted (rules-of-hooks). */
const EMPTY_PANEL: Cell = {
  i: "__jsonview_inactive__",
  x: 0,
  y: 0,
  w: 0,
  h: 0,
  widget_type: "chart",
  binding: { series: "" },
};

export function JsonView({ source, options, label, refreshKey = 0, cell, scope }: Props) {
  const flow = flowBindingOfSource(source);
  // Read views default to the `payload` field (flow-dashboard-binding-ux-scope); `options.envelope`
  // opts into showing the WHOLE `{payload, topic, …}` envelope. Both ride one `flows.node_state` read.
  // A picked JSON path (the visual builder, incl. "(whole value)") extracts exactly that field;
  // otherwise `options.envelope` shows the WHOLE envelope, else the port's `payload`. One read.
  const hasPath = flow?.path !== undefined;
  const wantEnvelope = !hasPath && options?.envelope === true;
  const flowState = useFlowNodeValue(
    flow?.flowId,
    flow?.node,
    flow?.port ?? "payload",
    wantEnvelope ? "output-envelope" : "output",
    refreshKey,
    flow?.path,
  );
  // The non-flow path: a plain tool source resolves through the one panel-data hook. Inactive (the
  // EMPTY placeholder) when the flow binding owns the view or there is no cell to read from.
  const panelState = usePanelData(!flow && cell ? cell : EMPTY_PANEL, scope ?? emptyScope(), refreshKey);

  // One row is the common tool-result shape ({tables, relationships, …} → a single object row):
  // unwrap it so the view shows the payload itself, not a one-element array around it.
  const panelValue = panelState.rows.length === 1 ? panelState.rows[0] : panelState.rows;
  const value = flow ? flowState.value : panelValue;
  const loading = flow ? flowState.loading : panelState.loading;
  const denied = flow ? flowState.denied : panelState.denied;

  // `options.collapsed` starts collapsed (a descriptor-declared render for a LARGE payload — e.g. a
  // datasource snapshot — posts itself minimized; the reader expands on demand).
  const [collapsed, setCollapsed] = useState(options?.collapsed === true);

  // Value mappings (false→on, 1→"ok", …) apply to a SCALAR value only — an object/array renders as
  // raw JSON. When a mapping hits, its `text` replaces the raw scalar and its `color` tints it, the
  // same fieldConfig bridge stat/gauge use (never a re-implemented match here). No cell / no mapping /
  // non-scalar value ⇒ the raw pretty-print, unchanged.
  const isScalar = value == null || typeof value !== "object";
  const mapped = cell && isScalar ? applyMappings(value, valueFieldOptions(cell).mappings) : null;

  return (
    <div className="flex h-full flex-col" aria-label={`json view ${flow?.node ?? label ?? ""}`}>
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

/** The collapsed one-liner. An object names its top keys with array sizes (`tables[6],
 *  relationships[6], truncated`) so a minimized snapshot still says what it holds. */
function summarize(v: unknown): string {
  if (v == null) return "null";
  if (Array.isArray(v)) return `[${v.length} items]`;
  if (typeof v === "object") {
    const entries = Object.entries(v as Record<string, unknown>);
    const parts = entries
      .slice(0, 5)
      .map(([k, val]) => (Array.isArray(val) ? `${k}[${val.length}]` : k));
    const more = entries.length > 5 ? ", …" : "";
    return `{${parts.join(", ")}${more}}`;
  }
  return String(v);
}
