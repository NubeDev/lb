// The host-mediated bridge handed to an extension page/widget's `mount(el, ctx, bridge)`
// (ui-federation scope). It is the ONLY way a page reaches platform data. The page calls
// `bridge.call(tool, args)`; the bridge filters the tool against the extension's granted `scope`
// (defense in depth) and forwards it through the shell's IPC seam to `POST /mcp/call`, where the host
// re-checks the capability + the workspace. The page NEVER receives the session token — the shell
// holds it; the bridge just carries a `{tool, args}` request, exactly as the scope froze.

import { openSeriesStream } from "@/lib/dashboard/series.stream";
import { invoke } from "@/lib/ipc/invoke";

/** What a mounted extension UI receives. `call` forwards a granted MCP tool; nothing else is reachable. */
export interface ExtBridge {
  /** Call one read-only MCP tool the extension was granted. Rejects locally if out of scope; the host
   *  re-checks regardless (a bypassed filter still hits a server-side deny). */
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
  /** Subscribe a live series feed the extension was granted (`series.watch`). Maps onto the shipped
   *  `GET /series/{series}/stream` SSE and delivers each `Sample` to `onEvent`; returns an unsubscribe.
   *  The control-engine canvas uses this for its live COV values: it arms the feed via
   *  `call('control-engine.watch')`, then streams the returned series here. Absent SSE (no gateway —
   *  Tauri/tests) yields a no-op unsubscribe and the page degrades to a static canvas, per its contract. */
  watch: (
    tool: string,
    args: Record<string, unknown>,
    onEvent: (e: unknown) => void,
  ) => () => void;
}

/** Build a bridge bound to `scope` — the extension's granted read-only tool set. */
export function makeBridge(scope: string[]): ExtBridge {
  const allowed = new Set(scope);
  return {
    call: async <T,>(tool: string, args?: Record<string, unknown>): Promise<T> => {
      if (!allowed.has(tool)) {
        // The host would deny it too; reject early so a misbehaving page gets a clear error.
        throw new Error(`out_of_scope: ${tool}`);
      }
      return invoke<T>("mcp_call", { tool, args: args ?? {} });
    },
    watch: (tool, args, onEvent) => {
      // Only `series.watch` is a streamable verb, and only if the extension was granted it (defense in
      // depth — the gateway's SSE route re-authenticates the session token regardless).
      if (tool !== "series.watch" || !allowed.has(tool)) return () => {};
      const series = typeof args.series === "string" ? args.series : "";
      if (series === "") return () => {};
      // Each SSE `Sample` IS the `{ payload, seq, ts, ... }` event the transport pulls its frame from.
      const stream = openSeriesStream(series, (sample) => onEvent(sample));
      return () => stream?.close();
    },
  };
}
