// The WidgetBridge v2 — the host-mediated bridge handed to a dashboard cell (widget-builder scope,
// "The widget contract, v2"). It is the page bridge NARROWED TO ONE CELL: the same `mount(el, ctx,
// bridge)` contract the shipped `proof-panel` page rides, but with the forwardable set widened from
// the frozen four series read verbs to `cell.tools ∩ install-grant` — which MAY include write tools.
//
// What did NOT change (load-bearing): the cell NEVER receives the session token. `call`/`watch` carry
// only `{tool, args}`; the shell holds the token and the host re-checks the capability + the workspace
// (from the token, never the cell or the iframe) on EVERY call. The local scope filter is defense in
// depth — a bypassed filter still hits a server-side deny.
//
// v2 adds `watch(tool, args, onEvent) => unsubscribe`: a streaming source (`series.watch` / `bus.watch`)
// satisfied by the SHIPPED series SSE (`GET /series/{s}/stream`) — no new transport, no polling. The
// stream tears down on unmount/uninstall (the caller invokes the returned unsubscribe).

import { invoke } from "@/lib/ipc/invoke";
import { openSeriesStream } from "@/lib/dashboard/series.stream";
import { openBusStream } from "@/lib/dashboard/bus.stream";

/** The cell's effective tool set = its declared `{source, action}` tools ∩ the install grant. The
 *  builder computes this; the bridge enforces it locally and the host re-enforces it server-side. */
export interface WidgetBridge {
  /** Call one tool in the cell's set (read OR write). Rejects locally if out of set; the host
   *  re-checks regardless. The token never appears in `args`. */
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
  /** Subscribe to a streaming source (`series.watch`/`bus.watch`). `onEvent` fires per live sample;
   *  the returned function unsubscribes (the renderer calls it on unmount). Returns a no-op when the
   *  tool is out of set or no gateway is configured (tests/Tauri) — the caller degrades to history. */
  watch: (
    tool: string,
    args: Record<string, unknown>,
    onEvent: (event: unknown) => void,
  ) => () => void;
}

/** The streaming verbs `watch` maps onto the series SSE. `series.watch` streams one named series;
 *  `bus.watch` streams a subject (same SSE mechanism over its subject — widget-builder scope, "Live
 *  feed"). Both take the streamed name from `args.series` / `args.subject`. */
const WATCH_VERBS = new Set(["series.watch", "bus.watch"]);

/** Build a WidgetBridge bound to `tools` — the cell's `{source, action}` tools ∩ grant. */
export function makeWidgetBridge(tools: string[]): WidgetBridge {
  const allowed = new Set(tools);
  return {
    call: async <T,>(tool: string, args?: Record<string, unknown>): Promise<T> => {
      if (!allowed.has(tool)) {
        // The host would deny it too (it's outside cell.tools ∩ grant); reject early with a clear
        // error so a misbehaving widget/template gets immediate feedback.
        throw new Error(`out_of_scope: ${tool}`);
      }
      return invoke<T>("mcp_call", { tool, args: args ?? {} });
    },
    watch: (tool, args, onEvent) => {
      if (!allowed.has(tool) || !WATCH_VERBS.has(tool)) return () => {};
      // `series.watch` streams a named series over `/series/{s}/stream`; `bus.watch` streams a generic
      // subject over `/bus/stream?subject=` (widget-config-vars "Platform fix"). Both attach the token
      // to the EventSource URL server-side — no token crosses into any widget-visible payload.
      if (tool === "bus.watch") {
        const subject = (args.subject as string | undefined) ?? "";
        if (!subject) return () => {};
        const stream = openBusStream(subject, (payload) => onEvent(payload));
        return () => stream?.close();
      }
      const series = (args.series as string | undefined) ?? "";
      if (!series) return () => {};
      const stream = openSeriesStream(series, (sample) => onEvent(sample));
      return () => stream?.close();
    },
  };
}
